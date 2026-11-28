//! One model panel: its surface, camera and mouse handling (navigation, picking, marquee, manipulator drags).

use aztek_scene::components::{self, ComponentMode};
use ash::vk;
use glam::{DMat4, DVec2, DVec3, Vec3};
use serde_json::{json, Value};

use crate::camera::CamKind;
use crate::core::Core;
use crate::text;
use crate::gpu::Gpu;
use crate::render::{c8, Kind, LineVertex, Push, Renderer};
use crate::viewport::{gather, Drag, Handle, Tool, Viewport};

pub const SHIFT: i32 = 1;
pub const CTRL: i32 = 2;
pub const ALT: i32 = 4;

/// One stroke of the paint skin weights brush: the weights as they build up. The stroke undoes as one.
struct PaintStroke {
    skin: aztek_scene::plug::NodeId,
    inf: u32,
    bind: aztek_scene::skin::SkinBind,
    /// With Skin Tools layers on the skin, the brush paints the current layer's target instead.
    layers: Option<(aztek_scene::skin_layers::Layers, u32, aztek_scene::skin_layers::Target)>,
    applied: bool,
    invert: bool,
    smooth: bool,
    last: Option<DVec3>,
}

/// One stroke of the paint attribute brush: a deformer's values as they build up, undoing as one.
struct AttrStroke {
    node: aztek_scene::plug::NodeId,
    slot: aztek_scene::deform_paint::Slot,
    weights: Vec<f64>,
    mask: Option<Vec<bool>>,
    applied: bool,
    invert: bool,
    smooth: bool,
    last: Option<DVec3>,
}

/// One stroke of a sculpting brush: the points as they build up, undoing as one.
struct SculptStroke {
    shape: aztek_scene::plug::NodeId,
    pos: Vec<DVec3>,
    base: Vec<DVec3>,
    mesh: std::sync::Arc<aztek_geo::Mesh>,
    world: DMat4,
    applied: bool,
    invert: bool,
    smooth: bool,
    last: Option<DVec3>,
}

/// The polygon creation tools' draft.
#[derive(Clone, Copy)]
struct PrimDraft {
    a: DVec3,
    b: DVec3,
    height: f64,
    stage: u8,
    press_y: f64,
}

/// A button or slider of the heads up display, placed.
struct HudPart {
    name: String,
    slider: bool,
    lo: DVec2,
    hi: DVec2,
    text: String,
    text_at: DVec2,
    min: f64,
    max: f64,
    value: f64,
    int: bool,
    precision: usize,
    /// The label's and the value's text sizes: -labelFontSize, -sliderLabelFontSize, -buttonLabelFontSize and
    /// -valueFontSize, small or large.
    text_size: f64,
    value_size: f64,
}

/// Which of the view's renderers draws the scene. The Renderer menu chooses it; both read the same scene and lights.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum RendererKind {
    /// Legacy in the menu: the reference's Viewport 2.0, the material shader with highlights and the fixed default
    /// lighting.
    Viewport2,
    /// Default in the menu, and what a view starts in: OpenPBR shading with one shadow map, the sky dome's image and
    /// ambient occlusion.
    #[default]
    Realtime,
}

impl RendererKind {
    pub fn parse(s: &str) -> Option<RendererKind> {
        Some(match s.to_ascii_lowercase().as_str() {
            "viewport2" | "viewport 2.0" | "viewport 2" | "vp2" | "legacy" => RendererKind::Viewport2,
            "realtime" | "real-time" | "real time" | "default" => RendererKind::Realtime,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            RendererKind::Viewport2 => "viewport2",
            RendererKind::Realtime => "realtime",
        }
    }
}

pub struct View {
    pub gpu: Gpu,
    pub r: Renderer,
    /// Which renderer the view's menu has chosen.
    pub renderer: RendererKind,
    /// The Realtime renderer's shadow map.
    realtime: Option<crate::realtime::Realtime>,
    /// What turns the drawn scene into the window's picture.
    post: crate::post::Post,
    /// Renderers of sample counts no longer in use: what was allocated against their layouts is still about.
    retired: Vec<Renderer>,
    pub vp: Viewport,
    /// Physical pixels per logical one.
    pub dpr: f64,
    nav: Option<(i32, DVec2)>,
    marquee: Option<DVec2>,
    joint_press: Option<DVec2>,
    /// The curves a joint placed with C held can land on, gathered once when the joint tool's button goes down.
    joint_curves: Vec<(DVec3, DVec3)>,
    /// The camera node this view looks through, by name, and what was last read from or written to it.
    pub cam_node: String,
    cam_sync: Option<[f64; 14]>,
    /// The modelling tools' state.
    pub ts: crate::tools::ToolState,
    /// A sculpt stroke in progress.
    sculpt: Option<SculptStroke>,
    /// A grooming stroke in progress, and the press's pixel for sizing the brush.
    groom: Option<aztek_scene::xjen::groom::Stroke>,
    /// A paint stroke in progress.
    stroke: Option<PaintStroke>,
    /// A paint attribute stroke in progress.
    attr_stroke: Option<AttrStroke>,
    /// B held with the paint tool: a drag sizes the brush.
    pub brush_adjust: bool,
    brush_drag: Option<(DVec2, f64)>,
    /// Lines a drag's preview step wrote to the history, retracted before the next step.
    drag_lines: usize,
    /// The drag's last preview command, run for real on release.
    last_drag: Option<(String, Value)>,
    /// Soft select's falloff being sized: B held and a drag.
    soft_drag: Option<(DVec2, f64)>,
    /// B held: where the mouse is, for the falloff circle.
    soft_cursor: Option<DVec2>,
    /// Whether B's hold did a drag, so its release doesn't also toggle.
    pub brush_dragged: bool,
    /// Tab held: a left drag selects what it passes over.
    pub tab_held: bool,
    /// The heads up display's control being pressed: its name, and whether it is the slider.
    hud_grab: Option<(String, bool)>,
    /// The ViewCube: the part under the mouse, and a press on it (part, where, whether it became a drag).
    cube_hover: Option<crate::view_cube::Part>,
    cube_press: Option<(crate::view_cube::Part, DVec2, bool)>,
    /// The paint skin weights tool is picking (its Select or Paint Select mode).
    skin_select: bool,
    /// The Bezier tool's anchors and the tangent end of each, world space, and whether the last is being dragged.
    bezier: Vec<(DVec3, DVec3)>,
    bezier_drag: bool,
    /// The cube and plane tools: first corner, far corner, height, and which stage: 1 dragging the base, 2 the height.
    prim: Option<PrimDraft>,
    /// The Snap Together tool's first click: object and world point.
    snap_first: Option<(String, DVec3)>,
    /// Where the time drag has got to.
    time_drag: f64,
    /// A lasso, drag select or paint select in progress.
    lassoing: bool,
    drag_selecting: bool,
    /// The last two components picked, for Shift+double-click's path: (mesh, kind, id).
    pick_hist: Vec<(String, String, u32)>,
    /// Camera history: [ and ] walk it.
    cam_undo: Vec<crate::camera::Camera>,
    cam_redo: Vec<crate::camera::Camera>,
    /// Frame rate for the heads up display.
    frame_times: std::collections::VecDeque<std::time::Instant>,
    pub background: [f32; 4],
    /// A parity capture: the picture alone, over the Render View's background. Black only when the scene has no dome.
    parity: bool,
    pub show_grid: bool,
    pub needs_redraw: bool,
    /// Last frame read back on request, as RGB.
    pub capture: Option<(u32, u32, Vec<u8>)>,
    pub want_capture: bool,
    /// Whether this view already said the device cannot draw the Realtime renderer.
    rt_warned: bool,
    /// A swapchain remake that failed: retry at the next frame rather than every time the window moves.
    resize_retry: bool,
    resize_reported: bool,
}

impl Drop for View {
    fn drop(&mut self) {
        // Nothing of the post pass's is in use once the queue is idle.
        unsafe {
            let _ = self.gpu.device.queue_wait_idle(self.gpu.queue);
        }
        self.post.destroy(&self.gpu);
        if let Some(rt) = self.realtime.take() {
            rt.release(&self.gpu);
        }
    }
}

impl View {
    pub fn new(gpu: Gpu, kind: CamKind, dpr: f64) -> View {
        let r = Renderer::new(&gpu);
        let post = crate::post::Post::new(&gpu);
        View {
            gpu,
            r,
            renderer: RendererKind::Realtime,
            realtime: None,
            post,
            retired: Vec::new(),
            vp: Viewport::new(kind),
            dpr,
            nav: None,
            marquee: None,
            joint_press: None,
            joint_curves: Vec::new(),
            cam_node: kind.name().to_string(),
            cam_sync: None,
            ts: Default::default(),
            sculpt: None,
            groom: None,
            stroke: None,
            attr_stroke: None,
            brush_adjust: false,
            brush_drag: None,
            drag_lines: 0,
            last_drag: None,
            soft_drag: None,
            soft_cursor: None,
            brush_dragged: false,
            tab_held: false,
            hud_grab: None,
            cube_hover: None,
            cube_press: None,
            skin_select: false,
            bezier: Vec::new(),
            bezier_drag: false,
            prim: None,
            snap_first: None,
            time_drag: 0.0,
            lassoing: false,
            drag_selecting: false,
            pick_hist: Vec::new(),
            cam_undo: Vec::new(),
            cam_redo: Vec::new(),
            frame_times: Default::default(),
            background: [0.36, 0.36, 0.36, 1.0],
            parity: false,
            show_grid: true,
            needs_redraw: true,
            capture: None,
            want_capture: false,
            rt_warned: false,
            resize_retry: false,
            resize_reported: false,
        }
    }

    /// Anti-aliasing on, off or at another strength, now: the view's pass, its pipelines and the post pass are made
    /// again for the new count.
    pub fn set_samples(&mut self, want: u32) {
        match self.gpu.set_samples(want) {
            Ok(true) => {
                self.retired.push(std::mem::replace(&mut self.r, Renderer::new(&self.gpu)));
                self.post.destroy(&self.gpu);
                self.post = crate::post::Post::new(&self.gpu);
                self.needs_redraw = true;
            }
            Ok(false) => {}
            Err(e) => log::error!("viewport anti-aliasing: {e}"),
        }
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        match self.gpu.recreate_swapchain(w, h) {
            Ok(()) => {
                self.resize_retry = false;
                self.resize_reported = false;
            }
            Err(e) => {
                // A remake can fail with a temporary condition (an out-of-date surface, a lost device being reset).
                // Keep the view alive, say so once, and try again at the next frame instead of dying or repeating.
                self.resize_retry = true;
                if !self.resize_reported {
                    self.resize_reported = true;
                    log::error!("viewport resize: {e}; retrying at the next frame");
                }
            }
        }
        self.needs_redraw = true;
    }

    /// Whether the scene has a sky dome: the Render View's background, and so the parity capture's.
    fn has_dome(&mut self, core: &mut Core) -> bool {
        crate::lighting::chosen_environment(&mut core.s.graph, crate::lighting::LightMode::All, crate::viewport::visible, core.s.scene_path.as_deref(), None).is_some()
    }

    pub fn render(&mut self, core: &mut Core) {
        if self.vp.tool == Tool::MultiCut {
            self.sync_multi_cut_scene(core);
        }
        let e = self.gpu.extent;
        self.vp.rect = [0.0, 0.0, e.width as f64, e.height as f64];
        // A parity capture is the picture alone, over the Render View's background: the scene's sky dome when it has
        // one (the view's own background, which is what the window shows), and black when it has none. Forcing black on
        // a domed scene darkened the capture, and every brightness comparison drawn from it.
        if self.parity && !self.has_dome(core) {
            self.background = [0.0, 0.0, 0.0, 1.0];
        }
        let bg = self.background;
        let prof = Prof::start();
        // A remake that failed last time is tried again here, before anything asks the swapchain for an image.
        if self.resize_retry {
            match self.gpu.recreate_swapchain(e.width, e.height) {
                Ok(()) => {
                    self.resize_retry = false;
                    self.resize_reported = false;
                }
                Err(_) => {
                    self.needs_redraw = true;
                    return;
                }
            }
        }
        let mut frame = self.gpu.begin_frame_deferred();
        prof.mark("  wait for the last frame");
        if frame.is_none() {
            // Out of date (the window changed size): remake at the window's size and try once more.
            self.resize(e.width, e.height);
            frame = self.gpu.begin_frame_deferred();
        }
        let e = self.gpu.extent;
        self.vp.rect = [0.0, 0.0, e.width as f64, e.height as f64];
        let Some(index) = frame else {
            self.needs_redraw = true;
            return;
        };
        // The last frame is done with them: the post pass reads this size's images.
        // (Bound after the lights are read: the reflections fall back on this frame's sky dome.)
        // The camera it looked through is gone (a new scene, a delete): back to the perspective camera.
        if !matches!(self.cam_node.as_str(), "persp" | "top" | "front" | "side") && !matches!(core.s.graph.find(&self.cam_node), Ok(Some(_))) {
            self.cam_node = "persp".to_string();
            self.vp.cam = crate::camera::Camera::new(CamKind::Persp);
            self.vp.reset_grid(&mut self.gpu);
            self.cam_sync = None;
        }
        self.read_camera(core);
        self.iso_in(core);
        let gathered = gather(&mut core.s.graph);
        let items = self.vp.filter_items(&core.s.graph, gathered);
        prof.mark("  gather");
        self.vp.show_grid = self.show_grid;
        if crate::ctx_cmds::wants_pin() {
            if let Some(p) = self.vp.manip_pivot(&mut core.s, &items) {
                crate::ctx_cmds::hold_pivot(p);
            }
        }
        self.vp.node_lines = if self.vp.tool == Tool::ShowManip && !self.brush_adjust { crate::node_manip::draw_lines(core, &self.vp, self.dpr) } else { Vec::new() };
        // B held: the falloff's reach as a circle on the surface under the mouse.
        if let (true, Some(px)) = (self.brush_adjust, self.soft_cursor) {
            let (centre, normal) = match self.vp.brush_hit(&core.s, &items, px) {
                Some((_, wp, n)) => (wp, n),
                None => (self.vp.place_point(px), self.vp.cam.dir_to_eye()),
            };
            let r = aztek_scene::soft_select::state().1;
            let (u, v) = normal.normalize_or_zero().any_orthonormal_pair();
            let at = |k: i32| centre + (u * (k as f64 / 64.0 * std::f64::consts::TAU).cos() + v * (k as f64 / 64.0 * std::f64::consts::TAU).sin()) * r;
            for k in 0..64 {
                self.vp.node_lines.push((at(k), at(k + 1), [255, 255, 80, 255]));
            }
        }
        self.vp.look_camera = core.s.graph.find(&self.cam_node).ok().flatten().and_then(|id| aztek_scene::cameras::shape_of(&core.s.graph, id)).unwrap_or(aztek_scene::plug::NO_NODE);
        self.vp.prepare_materials(&self.gpu, &mut core.s);
        let realtime = self.renderer == RendererKind::Realtime && self.r.realtime;
        if self.renderer == RendererKind::Realtime && !self.r.realtime && !self.rt_warned {
            self.rt_warned = true;
            core.push("// Warning: the Default renderer needs a graphics device with 256 bytes of push constants: drawing with Legacy //".to_string(), crate::core::LineKind::Warning);
        }
        self.vp.prepare_lights(&self.gpu, &self.r, &mut core.s, &items, realtime);
        // The post pass reads the scene's images, its material and the sky dome the reflections fall back on.
        let (env, env_rot, env_set) = self.vp.lit.as_ref().map(|l| l.environment()).unwrap_or(([0.0; 4], [0.0, 0.0, 0.0, 1.0], None));
        self.post.bind_images(&self.gpu, env_set);
        let (reflector, reflector_up, reflector_point) = ground_reflector(&items);
        let planar_on = realtime && reflector != aztek_scene::plug::NO_NODE;
        self.vp.planar_debug_color = std::env::var("AZTEK_PLANAR_DEBUG_COLOR").ok().map(|_| [0.0, 1.0, 0.0]);
        // What glass lets through: the scene without its transmissive surfaces, drawn from the camera's own view
        // and lights (#302). Nothing is drawn when no material transmits.
        if realtime && self.vp.needs_back() {
            self.vp.back = true;
            self.gpu.begin_back();
            self.vp.draw(&mut self.gpu, &self.r, &mut core.s, &items, self.dpr, realtime);
            self.gpu.end_back();
            self.vp.back = false;
        }
        // The ground plane's reflection: the scene mirrored below the world's ground, drawn before the camera's own view
        // so the floor can read what it reflects (#302).
        if planar_on {
            let up = DVec3::new(reflector_up[0] as f64, reflector_up[1] as f64, reflector_up[2] as f64);
            let point = DVec3::new(reflector_point[0], reflector_point[1], reflector_point[2]);
            let mut r = DMat4::IDENTITY;
            for i in 0..3 {
                for j in 0..3 {
                    r.col_mut(i)[j] = if i == j { 1.0 } else { 0.0 } - 2.0 * up[i] * up[j];
                }
            }
            let mirror = DMat4::from_translation(point) * r * DMat4::from_translation(-point);
            self.vp.planar_skip = reflector;
            self.vp.planar = Some(mirror);
            // The mirrored side facing the floor is the real underside, which the main light does not reach: a plain
            // fill stands in for the floor's bounce. The view stays the main one.
            let proj_main = self.vp.cam.proj(self.vp.aspect());
            self.vp.write_lights_for(&mut core.s, &items, self.vp.cam.view(), proj_main, self.vp.disp.light_mode);
            self.gpu.begin_planar();
            self.vp.draw(&mut self.gpu, &self.r, &mut core.s, &items, self.dpr, realtime);
            self.gpu.end_planar();
            self.vp.planar = None;
        }
        // The shadow map is drawn before the scene's pass, with the lights read.
        let mut glass = std::collections::HashSet::new();
        if realtime {
            if self.realtime.is_none() {
                self.realtime = Some(crate::realtime::Realtime::new(&self.gpu, self.r.rt_shadow_layout));
            }
            // A transmissive shape passes light instead of blocking it: drawing it into the shadow map would put an
            // opaque shadow under the glass and the glass would refract that black floor. Leave it out of the casters.
            glass = self.vp.transmissive_shapes(&items);
            if let Some(rt) = self.realtime.as_mut() {
                rt.render(&mut self.gpu, &items, self.vp.shadow, &glass);
            }
            // The material pipelines' set 4 has this renderer's four bindings: it is bound whether or not a shadow map
            // is drawn this frame, and the shader skips the sampling when no light casts one (#228 review).
            self.vp.rt_shadow_set = self.realtime.as_ref().and_then(|r| r.set());
        } else {
            self.vp.rt_shadow_set = None;
        }
        self.gpu.begin_scene([bg[0], bg[1], bg[2], 1.0]);
        self.vp.draw(&mut self.gpu, &self.r, &mut core.s, &items, self.dpr, realtime);
        // Glass's far side, for its second interface: drawn now that the scene's own meshes are built.
        if realtime {
            self.gpu.begin_backface();
            self.vp.draw_backfaces(&mut self.gpu, &self.r, &items, &glass);
        }
        prof.mark("  draw");
        if crate::usd_view::busy() {
            core.version += 1;
        }
        for w in crate::textures::with_cache(&self.gpu, |c| c.take_warnings()) {
            core.push(format!("// Warning: {w} //"), crate::core::LineKind::Warning);
        }
        // The guides go on a layer of their own, which the post pass lays over the finished picture (#300).
        self.gpu.begin_overlay();
        self.overlay(core, &items);
        prof.mark("  overlay");
        // The scene is drawn: it goes to the window through the post effects.
        let d = &self.vp.disp;
        let rgb = |n: &str| aztek_scene::system_cmds::view_rgb(n).map(|c| [c[0] as f32, c[1] as f32, c[2] as f32]).unwrap_or([0.0; 3]);
        let gradient = aztek_scene::system_cmds::display_gradient(&core.s).then(|| (rgb("backgroundTop"), rgb("backgroundBottom")));
        let contact = if realtime && d.shadows && !d.xray { self.vp.contact } else { None };
        let (proj, ortho) = (self.vp.cam.proj(self.vp.aspect()).as_mat4(), self.vp.cam.ortho());
        // Depth of field follows the camera looked through, as Harold does: on only where its depthOfField is.
        let dof = if d.depth_of_field && !d.xray && !ortho && self.vp.look_camera != aztek_scene::plug::NO_NODE {
            aztek_scene::harold::camera_lens(&mut core.s.graph, self.vp.look_camera)
        } else {
            None
        };
        let settings = crate::post::PostSettings {
            ambient_occlusion: d.ambient_occlusion && !d.xray,
            ao_radius: d.ao_radius as f32,
            ao_intensity: d.ao_intensity as f32,
            gradient,
            clear_grey: bg[0],
            contact,
            high_quality: realtime,
            bounce: if realtime && d.bounce_light && !d.xray { 1.0 } else { 0.0 },
            dof,
            reflections: realtime && !d.xray && std::env::var("AZTEK_REFLECTIONS").map_or(true, |v| v != "0"),
            debug_reflection: std::env::var("AZTEK_REFLECTION_DEBUG").ok().and_then(|v| v.parse().ok()).unwrap_or(0),
            planar_reflection: planar_on,
            up: reflector_up,
            view: self.vp.cam.view().as_mat4(),
            env,
            env_rot,
            bg_env: match &self.vp.environment {
                Some(e) if e.image.is_some() => [1.0, 1.0, 1.0, 2.0],
                Some(e) => [e.color[0], e.color[1], e.color[2], 1.0],
                None => [0.0; 4],
            },
        };
        // Reflections and refraction, from the scene's colour, depth and material, before the picture is put together.
        self.gpu.begin_reflection();
        self.post.draw_reflection(&self.gpu, proj, ortho, &settings);
        self.gpu.end_reflection();
        // A small low-pass of the reflection image, so one bright texel does not come back as a speckle (#302), and
        // the filtered copies the gather picks a level from by the surface's roughness.
        if settings.reflections {
            self.gpu.begin_refl_blur();
            self.post.draw_reflection_blur(&self.gpu, proj, ortho, &settings);
            self.gpu.end_refl_blur();
            self.gpu.build_reflection_mips();
        }
        self.gpu.begin_occlusion_after();
        self.post.draw_occlusion(&self.gpu, proj, ortho, &settings);
        if settings.bounce > 0.0 {
            self.gpu.begin_bounce();
            self.post.draw_bounce(&self.gpu, proj, ortho, &settings);
            // The bounce's rays are stochastic: low-pass it along each axis into a soft, continuous field before the
            // picture reads it, so no single hit comes back as a bright texel (#302).
            self.gpu.begin_bounce_blur_h();
            self.post.draw_bounce_blur(&self.gpu, proj, ortho, &settings, true);
            self.gpu.begin_bounce_blur_v();
            self.post.draw_bounce_blur(&self.gpu, proj, ortho, &settings, false);
            // The pass stays open: begin_post ends whichever pass is current.
        }
        self.gpu.begin_post(index);
        self.post.draw(&self.gpu, proj, ortho, &settings);
        let ok = self.gpu.end_frame(index);
        prof.mark("  submit");
        if self.want_capture {
            self.want_capture = false;
            match self.gpu.read_back(index) {
                Ok(c) => self.capture = Some(c),
                Err(e) => log::error!("capture: {e}"),
            }
        }
        if !ok {
            let (w, h) = (e.width, e.height);
            self.resize(w, h);
        }
        self.needs_redraw = self.vp.drag.is_some() && false;
    }

    /// Component commands act in what the view the user is in shows.
    fn iso_in(&self, core: &mut Core) {
        core.s.graph.active_isolation = self.vp.iso.active().then(|| self.vp.iso.clone());
    }

    /// The heads up displays' rows by screen section: 0 top left, 2 top centre, 4 top right, 5 bottom left, 7 bottom centre,
    /// 9 bottom right.
    fn hud_rows(&mut self, core: &mut Core, items: &[crate::viewport::DrawItem]) -> Vec<(i64, String)> {
        use crate::ui_cmds::hud_on;
        let mut rows = Vec::new();
        if hud_on("polyCount") {
            let sel = core.s.graph.selection().to_vec();
            let mut all = [0u64; 4];
            let mut picked = [0u64; 4];
            for it in items {
                let m = &it.mesh;
                let (mut v, mut e, mut f, mut t) = (0u64, 0u64, 0u64, 0u64);
                for p in 0..m.point_capacity() {
                    v += m.point_valid(p) as u64;
                }
                for i in 0..m.edge_capacity() {
                    e += m.edge_valid(i) as u64;
                }
                for i in 0..m.face_capacity() {
                    if m.face_valid(i) {
                        f += 1;
                        t += (m.face_points(i).len() as u64).saturating_sub(2);
                    }
                }
                for (k, x) in [v, e, f, t].into_iter().enumerate() {
                    all[k] += x;
                    if sel.contains(&it.xf) || sel.contains(&it.shape) {
                        picked[k] += x;
                    }
                }
            }
            for (k, name) in ["Verts:", "Edges:", "Faces:", "Tris:"].iter().enumerate() {
                rows.push((0, format!("{name:<7}{:>8}{:>8}", all[k], picked[k])));
            }
        }
        if hud_on("objectDetails") {
            let g = &core.s.graph;
            if let Some(&lead) = g.selection().last() {
                if let Some(n) = g.node(lead) {
                    rows.push((4, format!("Object: {}", n.name())));
                    rows.push((4, format!("Type: {}", n.ty().name)));
                }
            }
        }
        if hud_on("selection") {
            let g = &core.s.graph;
            let c = aztek_scene::components::component_strings(g, true).len();
            rows.push((2, format!("Selected: {}", if c > 0 { c } else { g.selection().len() })));
        }
        if hud_on("frameRate") {
            let now = std::time::Instant::now();
            self.frame_times.push_back(now);
            while self.frame_times.len() > 30 {
                self.frame_times.pop_front();
            }
            let fps = match self.frame_times.front() {
                Some(a) if self.frame_times.len() > 1 => (self.frame_times.len() - 1) as f64 / (now - *a).as_secs_f64().max(1e-3),
                _ => 0.0,
            };
            rows.push((9, format!("{fps:.1} fps")));
        }
        if hud_on("currentFrame") {
            rows.push((9, format!("Frame: {}", aztek_scene::anim::frame(&core.s.graph))));
        }
        if self.vp.iso.active() {
            rows.push((2, "Isolate Select".to_string()));
        }
        rows
    }

    /// The scripted heads up display blocks, the presets' values worked out.
    fn hud_blocks(&mut self, core: &mut Core, items: &[crate::viewport::DrawItem]) -> Vec<crate::hud::Block> {
        let mut blocks = crate::hud::blocks(core);
        if !blocks.iter().any(|b| !b.preset.is_empty()) {
            return blocks;
        }
        let sel = core.s.graph.selection().to_vec();
        // Totals over what the view shows, over the selected objects, and the selected components.
        let mut all = [0u64; 5];
        let mut picked = [0u64; 5];
        for it in items {
            let m = &it.mesh;
            let (mut v, mut e, mut f, mut t) = (0u64, 0u64, 0u64, 0u64);
            for p in 0..m.point_capacity() {
                v += m.point_valid(p) as u64;
            }
            for i in 0..m.edge_capacity() {
                e += m.edge_valid(i) as u64;
            }
            for i in 0..m.face_capacity() {
                if m.face_valid(i) {
                    f += 1;
                    t += (m.face_points(i).len() as u64).saturating_sub(2);
                }
            }
            let path = core.s.graph.path(it.shape);
            let uv = core.s.run("polyEvaluate", json!({"args": [path], "uvcoord": true})).ok().and_then(|v| v.as_u64()).unwrap_or(0);
            for (k, x) in [v, e, f, uv, t].into_iter().enumerate() {
                all[k] += x;
                if sel.contains(&it.xf) || sel.contains(&it.shape) {
                    picked[k] += x;
                }
            }
        }
        let active = |core: &mut Core, flag: &str| core.s.run("polyEvaluate", json!({ flag: true })).ok().and_then(|v| v.as_u64()).unwrap_or(0);
        for b in blocks.iter_mut().filter(|b| !b.preset.is_empty()) {
            b.data = match b.preset.as_str() {
                "polyVerts" | "polyEdges" | "polyFaces" | "polyUVs" | "polyTriangles" => {
                    let (k, flag) = match b.preset.as_str() {
                        "polyVerts" => (0, "vertexComponent"),
                        "polyEdges" => (1, "edgeComponent"),
                        "polyFaces" => (2, "faceComponent"),
                        "polyUVs" => (3, "uvComponent"),
                        _ => (4, "triangleComponent"),
                    };
                    vec![all[k].to_string(), picked[k].to_string(), active(core, flag).to_string()]
                }
                "cameraNames" => vec![self.cam_node.clone()],
                "frameRate" => {
                    let now = std::time::Instant::now();
                    self.frame_times.push_back(now);
                    while self.frame_times.len() > 30 {
                        self.frame_times.pop_front();
                    }
                    let fps = match self.frame_times.front() {
                        Some(a) if self.frame_times.len() > 1 => (self.frame_times.len() - 1) as f64 / (now - *a).as_secs_f64().max(1e-3),
                        _ => 0.0,
                    };
                    vec![format!("{fps:.1} fps")]
                }
                "distanceFromCamera" => {
                    let names: Vec<Value> = sel.iter().map(|&i| json!(core.s.graph.path(i))).collect();
                    let bb = if names.is_empty() { Value::Null } else { core.s.run("exactWorldBoundingBox", json!({"args": names})).unwrap_or(Value::Null) };
                    let v: Vec<f64> = bb.as_array().into_iter().flatten().filter_map(Value::as_f64).collect();
                    if v.len() >= 6 {
                        let c = DVec3::new(v[0] + v[3], v[1] + v[4], v[2] + v[5]) * 0.5;
                        vec![format!("{:.*}", b.precision, self.vp.cam.eye().distance(c))]
                    } else {
                        vec![String::new()]
                    }
                }
                // Nothing loads in the background here.
                "materialLoadingCount" | "textureLoadingCount" => vec!["0".to_string()],
                _ => Vec::new(),
            };
        }
        blocks
    }

    /// Where the script-made controls of the heads up display sit: a section's column, a block's row.
    fn hud_parts(&self) -> Vec<HudPart> {
        let [_, _, w, h] = self.vp.rect;
        let s = self.dpr;
        let size = 9.0 * s;
        let row_h = 18.0 * s;
        let num = |c: &Value, k: &str, d: f64| c.get(k).and_then(Value::as_f64).unwrap_or(d);
        let txt = |c: &Value, k: &str| c.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let mut out = Vec::new();
        for (name, c) in crate::ui_cmds::hud_controls() {
            let section = num(&c, "section", 0.0) as i64;
            let block = num(&c, "block", 0.0);
            let kind = txt(&c, "kind");
            let label = if kind == "hudSliderButton" { txt(&c, "sliderLabel") } else { txt(&c, "label") };
            let large = |k: &str| if txt(&c, k) == "large" { size * 1.3 } else { size };
            let label_size = large(if kind == "hudSliderButton" { "sliderLabelFontSize" } else { "labelFontSize" });
            let button_size = large(if kind == "hudSliderButton" { "buttonLabelFontSize" } else { "labelFontSize" });
            let value_size = large("valueFontSize");
            let has_slider = kind != "hudButton";
            let has_button = kind != "hudSlider";
            let button_text = if kind == "hudButton" { label.clone() } else { txt(&c, "buttonLabel") };
            let button_w = num(&c, "buttonWidth", 0.0).max(0.0) * s;
            let button_w = if button_w > 0.0 { button_w } else { text::width(&button_text, button_size) + 12.0 * s };
            let track = if kind == "hudSliderButton" { num(&c, "sliderLength", 80.0) } else { num(&c, "sliderLength", 100.0) } * s;
            let label_w = if has_slider { text::width(&label, label_size) + 6.0 * s } else { 0.0 };
            let total = label_w + if has_slider { track + 52.0 * s } else { 0.0 } + if has_button { button_w + 4.0 * s } else { 0.0 };
            let x0 = match section % 5 {
                0 | 1 => 10.0 * s,
                2 => w * 0.5 - total * 0.5,
                _ => w - total - 10.0 * s,
            };
            let y = if section < 5 { 16.0 * s + block * row_h } else { h - 26.0 * s - block * row_h };
            let mut x = x0;
            if has_slider {
                let at = DVec2::new(x + label_w, y - 5.0 * s);
                out.push(HudPart { name: name.clone(), slider: true, lo: at, hi: at + DVec2::new(track, 12.0 * s), text: label.clone(), text_at: DVec2::new(x, y), min: num(&c, "minValue", 0.0), max: num(&c, "maxValue", 100.0), value: num(&c, "value", 0.0), int: txt(&c, "type") == "int", precision: num(&c, "decimalPrecision", 2.0) as usize, text_size: label_size, value_size });
                x += label_w + track + 52.0 * s;
            }
            if has_button {
                let at = DVec2::new(x, y - 7.0 * s);
                out.push(HudPart { name: name.clone(), slider: false, lo: at, hi: at + DVec2::new(button_w, 14.0 * s), text: button_text, text_at: DVec2::new(x + 6.0 * s, y), min: 0.0, max: 0.0, value: 0.0, int: false, precision: 0, text_size: button_size, value_size: size });
            }
        }
        out
    }

    /// A press, drag or release on the heads up display's controls. True when it was theirs.
    fn hud_mouse(&mut self, core: &mut Core, kind: i32, button: i32, px: DVec2) -> bool {
        let run = |core: &mut Core, name: &str, flags: &[&str]| {
            let c = crate::ui_cmds::hud_controls().into_iter().find(|(n, _)| n == name).map(|x| x.1);
            for f in flags {
                if let Some(cb) = c.as_ref().and_then(|c| c.get(*f)).cloned() {
                    core.run_callback(&cb, &[]);
                }
            }
        };
        let set = |p: &HudPart, x: f64| {
            let t = ((x - p.lo.x) / (p.hi.x - p.lo.x).max(1.0)).clamp(0.0, 1.0);
            let v = p.min + (p.max - p.min) * t;
            crate::ui_cmds::hud_set_value(&p.name, if p.int { v.round() } else { v });
        };
        match kind {
            0 if button == 1 => {
                let Some(p) = self.hud_parts().into_iter().find(|p| px.x >= p.lo.x && px.x <= p.hi.x && px.y >= p.lo.y && px.y <= p.hi.y) else { return false };
                if p.slider {
                    set(&p, px.x);
                    run(core, &p.name, &["pressCommand", "sliderPressCommand"]);
                } else {
                    run(core, &p.name, &["pressCommand", "buttonPressCommand"]);
                }
                self.hud_grab = Some((p.name, p.slider));
                self.needs_redraw = true;
                true
            }
            1 => {
                let Some((name, slider)) = self.hud_grab.clone() else { return false };
                if slider {
                    if let Some(p) = self.hud_parts().into_iter().find(|p| p.slider && p.name == name) {
                        set(&p, px.x);
                        run(core, &name, &["dragCommand", "sliderDragCommand"]);
                    }
                }
                self.needs_redraw = true;
                true
            }
            2 => {
                let Some((name, slider)) = self.hud_grab.take() else { return false };
                let flags: [&str; 2] = if slider { ["releaseCommand", "sliderReleaseCommand"] } else { ["releaseCommand", "buttonReleaseCommand"] };
                run(core, &name, &flags);
                self.needs_redraw = true;
                true
            }
            _ => false,
        }
    }

    pub fn on_cube(&self, px: DVec2) -> bool {
        let [_, _, w, h] = self.vp.rect;
        crate::view_cube::layout(w, h, self.dpr, &self.vp.cam).is_some_and(|c| c.hit(px).is_some())
    }

    /// The ViewCube under the mouse: hover, a click on a part, a drag to turn the camera. None when the mouse is not
    /// the cube's.
    fn cube_mouse(&mut self, core: &mut Core, kind: i32, button: i32, px: DVec2, buttons: i32) -> Option<bool> {
        use crate::view_cube::Part;
        let [_, _, w, h] = self.vp.rect;
        let cube = crate::view_cube::layout(w, h, self.dpr, &self.vp.cam)?;
        match kind {
            0 if button == 1 => {
                let part = cube.hit(px)?;
                self.cube_press = Some((part, px, false));
                Some(true)
            }
            // A right click on the cube: its menu, as the triangle gives it.
            0 if button == 3 => {
                cube.hit(px)?;
                let ortho = self.vp.cam.orthographic;
                crate::core::push_ui_request(format!("viewCubeMenu|{}", json!({"orthographic": ortho, "camera": self.cam_node})));
                Some(true)
            }
            1 if self.cube_press.is_some() && buttons & 1 != 0 => {
                let (part, at, _) = self.cube_press.unwrap();
                let d = px - at;
                if d.length() > 4.0 * self.dpr || self.cube_press.is_some_and(|p| p.2) {
                    // A drag on the cube turns the camera about its centre of interest.
                    let up = crate::view_cube::flag("preserveSceneUp");
                    let c = &mut self.vp.cam;
                    c.yaw -= d.x * 0.5;
                    c.pitch = (c.pitch + d.y * 0.5).clamp(-89.9, 89.9);
                    if up {
                        c.roll = 0.0;
                    }
                    self.cube_press = Some((part, px, true));
                    self.write_camera(core);
                }
                Some(true)
            }
            2 if self.cube_press.is_some() => {
                let (part, _, dragged) = self.cube_press.take().unwrap();
                if dragged && crate::view_cube::flag("dragSnap") {
                    // Snap to closest view: the drag ends on the nearest of the cube's views.
                    let r = crate::view_cube::nearest_region(&cube, self.vp.cam.dir_to_eye());
                    let (yaw, pitch) = crate::camera::Camera::angles_of(cube.direction(r));
                    let c = &mut self.vp.cam;
                    c.yaw = yaw;
                    c.pitch = pitch;
                    c.roll = 0.0;
                    self.write_camera(core);
                }
                if !dragged && button == 1 && cube.hit(px) == Some(part) {
                    match part {
                        Part::Region(r) => {
                            self.cam_push();
                            let (yaw, pitch) = crate::camera::Camera::angles_of(cube.direction(r));
                            let c = &mut self.vp.cam;
                            c.yaw = yaw;
                            c.pitch = pitch;
                            c.roll = 0.0;
                            self.write_camera(core);
                            // Fit-to-view on view change.
                            if crate::view_cube::flag("zoomToFitScene") {
                                let _ = core.exec("viewFit", json!({}));
                            }
                        }
                        Part::Home => self.cube_action(core, "home"),
                        Part::Menu => {
                            let ortho = self.vp.cam.orthographic;
                            crate::core::push_ui_request(format!("viewCubeMenu|{}", json!({"orthographic": ortho, "camera": self.cam_node})));
                        }
                    }
                }
                Some(true)
            }
            1 if buttons == 0 => {
                let part = cube.hit(px);
                if part != self.cube_hover {
                    self.cube_hover = part;
                    self.needs_redraw = true;
                }
                // Over the cube the mouse is its own; elsewhere the view's.
                part.map(|_| true)
            }
            _ => None,
        }
    }

    /// The ViewCube's actions on this view's camera: home, the default home, set home, level, perspective or
    /// orthographic, set or reset the front.
    pub fn cube_action(&mut self, core: &mut Core, action: &str) {
        let cam = self.cam_node.clone();
        match action {
            "home" | "default" => {
                self.cam_push();
                let home = if action == "home" { crate::view_cube::home_of(&cam) } else { None };
                let k = self.vp.cam.kind;
                let mut c = home.unwrap_or_else(|| crate::camera::Camera::new(k));
                c.orthographic = self.vp.cam.orthographic;
                self.vp.cam = c;
                self.write_camera(core);
            }
            "setHome" => crate::view_cube::set_home(&cam, Some(self.vp.cam.clone())),
            "resetHome" => crate::view_cube::set_home(&cam, None),
            "level" => {
                self.vp.cam.pitch = 0.0;
                self.vp.cam.roll = 0.0;
                self.write_camera(core);
            }
            "perspective" | "orthographic" => {
                let ortho = action == "orthographic";
                if self.vp.cam.orthographic != ortho {
                    let c = &mut self.vp.cam;
                    // As wide as the perspective view shows at its centre of interest, and back.
                    if ortho {
                        c.ortho_width = 2.0 * c.distance * (c.hfov.to_radians() * 0.5).tan();
                    }
                    c.orthographic = ortho;
                    let path = self.cam_node.clone();
                    let g = &mut core.s.graph;
                    if let Ok(Some(id)) = g.find(&path) {
                        let shape = aztek_scene::cameras::shape_of(g, id).unwrap_or(id);
                        let _ = g.set_value(aztek_scene::plug::PlugRef::new(shape, aztek_scene::cameras::ORTHOGRAPHIC as i32), aztek_scene::plug::PlugValue::Bool(ortho));
                    }
                    self.write_camera(core);
                }
            }
            "setFront" => {
                // The view's direction, along the ground, becomes the front.
                let (yaw, _) = crate::camera::Camera::angles_of(self.vp.cam.dir_to_eye());
                crate::view_cube::set_front(yaw);
            }
            "resetFront" => crate::view_cube::set_front(0.0),
            _ => {}
        }
        self.needs_redraw = true;
    }

    /// Screen-space extras: marquee, lasso, gates, heads up display, axis triad, camera name, numeric readout.
    /// The painted influence's weight beside each point of the hilited skinned meshes no surface hides, as the paint
    /// skin weights tool's Show Numbers draws them.
    fn weight_numbers(&mut self, core: &Core, items: &[crate::viewport::DrawItem]) -> Vec<(DVec2, String, [u8; 4])> {
        let ctx = &core.s.paint_ctx;
        if self.vp.tool != Tool::PaintSkin || !ctx.numeric {
            return Vec::new();
        }
        let col = ctx.numeric_color.map(|x| (x.clamp(0.0, 1.0) * 255.0) as u8);
        let col = [col[0], col[1], col[2], 255];
        let decimals = ctx.numeric_precision.clamp(0, 6) as usize;
        let screen = self.vp.screen();
        let sel = core.s.graph.selection().to_vec();
        let mut out = Vec::new();
        for it in items {
            if !sel.contains(&it.xf) && !sel.contains(&it.shape) {
                continue;
            }
            let Some(w) = aztek_scene::skin::influence_weights(&core.s, it.shape) else { continue };
            let m = &it.mesh;
            // Too many to read: none.
            if m.num_points() > 4000 {
                continue;
            }
            let one = [crate::viewport::DrawItem { shape: it.shape, xf: it.xf, world: it.world, mesh: it.mesh.clone(), smooth: 0 }];
            for p in 0..m.point_capacity() {
                if !m.point_valid(p) {
                    continue;
                }
                let wp = it.world.transform_point3(m.position(p));
                let Some(q) = screen.project(wp) else { continue };
                if !self.vp.visible(&one, wp, &screen) {
                    continue;
                }
                out.push((q, format!("{:.*}", decimals, w.get(p as usize).copied().unwrap_or(0.0)), col));
            }
        }
        out
    }

    fn overlay(&mut self, core: &mut Core, items: &[crate::viewport::DrawItem]) {
        let hud_shown = crate::hud::layout_visible();
        let hud = if hud_shown { self.hud_rows(core, items) } else { Vec::new() };
        let hud_blocks = if hud_shown { self.hud_blocks(core, items) } else { Vec::new() };
        let hud_parts = if hud_shown { self.hud_parts() } else { Vec::new() };
        let readout = self.readout();
        let gate = self.gate_aspects(core);
        let numbers = self.weight_numbers(core, items);
        let [_, _, w, h] = self.vp.rect;
        if w < 2.0 || h < 2.0 {
            return;
        }
        let mut lines: Vec<LineVertex> = Vec::new();
        let mut seg = |a: DVec2, b: DVec2, c: [u8; 4]| {
            lines.push(LineVertex { pos: [a.x as f32, a.y as f32, 0.5], color: c8(c) });
            lines.push(LineVertex { pos: [b.x as f32, b.y as f32, 0.5], color: c8(c) });
        };
        if let Some((a, b)) = self.vp.marquee {
            let (lo, hi) = (a.min(b), a.max(b));
            let c = [255, 255, 255, 255];
            seg(lo, DVec2::new(hi.x, lo.y), c);
            seg(DVec2::new(hi.x, lo.y), hi, c);
            seg(hi, DVec2::new(lo.x, hi.y), c);
            seg(DVec2::new(lo.x, hi.y), lo, c);
        }
        for pair in self.vp.lasso.windows(2) {
            seg(pair[0], pair[1], [255, 255, 255, 255]);
        }
        if let Some((c, r)) = self.vp.select_brush {
            let n = 32;
            for k in 0..n {
                let (a0, a1) = (k as f64 / n as f64 * std::f64::consts::TAU, (k + 1) as f64 / n as f64 * std::f64::consts::TAU);
                seg(c + DVec2::new(a0.cos(), a0.sin()) * r, c + DVec2::new(a1.cos(), a1.sin()) * r, [255, 220, 90, 255]);
            }
        }
        // Gates: the film and resolution frames, fitted to the view, and the safe areas inside the latter.
        let frame_rect = |aspect: f64, inset: f64| {
            let (mut rw, mut rh) = (w * 0.9, w * 0.9 / aspect);
            if rh > h * 0.9 {
                rh = h * 0.9;
                rw = rh * aspect;
            }
            let (rw, rh) = (rw * inset, rh * inset);
            (DVec2::new((w - rw) * 0.5, (h - rh) * 0.5), DVec2::new((w + rw) * 0.5, (h + rh) * 0.5))
        };
        let mut rect = |lo: DVec2, hi: DVec2, c: [u8; 4]| {
            seg(lo, DVec2::new(hi.x, lo.y), c);
            seg(DVec2::new(hi.x, lo.y), hi, c);
            seg(hi, DVec2::new(lo.x, hi.y), c);
            seg(DVec2::new(lo.x, hi.y), lo, c);
        };
        if self.vp.disp.film_gate {
            let (a, b) = frame_rect(gate.0, 1.0);
            rect(a, b, [190, 190, 190, 255]);
        }
        if self.vp.disp.res_gate {
            let (a, b) = frame_rect(gate.1, 1.0);
            rect(a, b, [230, 230, 230, 255]);
        }
        if self.vp.disp.safe_action {
            let (a, b) = frame_rect(gate.1, 0.9);
            rect(a, b, [190, 190, 120, 255]);
        }
        let mut seg = |a: DVec2, b: DVec2, c: [u8; 4]| {
            lines.push(LineVertex { pos: [a.x as f32, a.y as f32, 0.5], color: c8(c) });
            lines.push(LineVertex { pos: [b.x as f32, b.y as f32, 0.5], color: c8(c) });
        };
        let s = self.dpr;
        // The ViewCube.
        let mut tris: Vec<LineVertex> = Vec::new();
        if let Some(cube) = crate::view_cube::layout(w, h, s, &self.vp.cam) {
            let mut tri = |a: DVec2, b: DVec2, c: DVec2, col: [u8; 4]| {
                for q in [a, b, c] {
                    tris.push(LineVertex { pos: [q.x as f32, q.y as f32, 0.5], color: c8(col) });
                }
            };
            let active = self.cube_hover.is_some() || self.cube_press.is_some();
            let mut words: Vec<LineVertex> = Vec::new();
            let mut word = |t: &str, at: DVec2, size: f64, col: [u8; 4]| text::draw(t, at, size, c8(col), &mut words);
            cube.draw(self.cube_hover, active, &mut tri, &mut seg, &mut word, 8.0 * s);
            tris.extend(words);
        }
        // The paint skin weights tool's Show Numbers.
        for (at, text, col) in &numbers {
            text::label(text, *at + DVec2::new(4.0 * s, -3.0 * s), 6.0 * s, c8(*col), &mut tris);
        }
        // Axis triad, bottom left.
        if hud_shown && crate::ui_cmds::hud_on("viewAxis") {
            let origin = DVec2::new(34.0 * s, h - 34.0 * s);
            let view = self.vp.cam.view();
            let axes = [(DVec3::X, [230, 50, 50, 255], 'x'), (DVec3::Y, [60, 220, 60, 255], 'y'), (DVec3::Z, [70, 110, 255, 255], 'z')];
            for (ax, c, name) in axes {
                let v = view.transform_vector3(ax);
                let tip = origin + DVec2::new(v.x, -v.y) * 20.0 * s;
                seg(origin, tip, c);
                let lp = origin + DVec2::new(v.x, -v.y) * 27.0 * s - DVec2::new(3.0, -4.0) * s;
                text::label(&name.to_string(), lp, 7.0 * s, c8(c), &mut tris);
            }
        }
        // Camera name, bottom centre.
        if hud_shown && crate::ui_cmds::hud_on("cameraNames") {
            let name = self.cam_node.as_str();
            let size = 11.0 * s;
            let tw = text::width(name, size);
            text::label(name, DVec2::new(w * 0.5 - tw * 0.5, h - 12.0 * s), size, c8([225, 225, 225, 255]), &mut tris);
        }
        // The scripted blocks: each section a fifth of the view across, its blocks down from the top or up from the bottom.
        let col_w = w / 5.0;
        let mut used_px = [0.0f64; 10];
        for section in 0..10i64 {
            let mut mine: Vec<&crate::hud::Block> = hud_blocks.iter().filter(|b| b.section == section).collect();
            mine.sort_by_key(|b| b.block);
            let Some(last) = mine.last().map(|b| b.block) else { continue };
            let height_of = |k: i64| mine.iter().find(|b| b.block == k).map_or(20.0, |b| b.height) * s;
            used_px[section as usize] = (0..=last).map(height_of).sum::<f64>();
            for b in &mine {
                let below: f64 = (0..b.block).map(height_of).sum();
                let bh = b.height * s;
                let top = if section < 5 { below } else { h - below - bh };
                let (lsize, dsize) = (if b.label_large { 13.0 } else { 10.0 } * s, if b.data_large { 13.0 } else { 10.0 } * s);
                let fit = |t: &str, width: f64, size: f64| -> String {
                    if width <= 0.0 {
                        return t.to_string();
                    }
                    let mut out = t.to_string();
                    while !out.is_empty() && text::width(&out, size) > width {
                        out.pop();
                    }
                    out
                };
                let label_w = if b.label_width > 0.0 { b.label_width * s } else if b.label.is_empty() { 0.0 } else { text::width(&b.label, lsize) + 6.0 * s };
                let data_w: Vec<f64> = b.data.iter().map(|d| if b.data_width > 0.0 { b.data_width * s } else { text::width(d, dsize) + 6.0 * s }).collect();
                let total = if b.preset == "viewAxis" { 50.0 * s } else { label_w + data_w.iter().sum::<f64>() };
                let (lo, hi) = (col_w * (section % 5) as f64 + b.padding * s, col_w * ((section % 5) + 1) as f64 - b.padding * s);
                let x0 = match b.align.as_str() {
                    "right" => hi - total,
                    "center" => (lo + hi) * 0.5 - total * 0.5,
                    _ => lo,
                };
                let base = top + bh * 0.5 + 4.0 * s;
                if b.preset == "viewAxis" {
                    let origin = DVec2::new(x0 + 25.0 * s, top + 25.0 * s);
                    let view = self.vp.cam.view();
                    for (ax, c, name) in [(DVec3::X, [230, 50, 50, 255], 'x'), (DVec3::Y, [60, 220, 60, 255], 'y'), (DVec3::Z, [70, 110, 255, 255], 'z')] {
                        let v = view.transform_vector3(ax);
                        seg(origin, origin + DVec2::new(v.x, -v.y) * 18.0 * s, c);
                        let lp = origin + DVec2::new(v.x, -v.y) * 24.0 * s - DVec2::new(3.0, -4.0) * s;
                        text::label(&name.to_string(), lp, 7.0 * s, c8(c), &mut tris);
                    }
                    continue;
                }
                let white = c8([230, 230, 230, 255]);
                if !b.label.is_empty() {
                    text::label(&fit(&b.label, label_w, lsize), DVec2::new(x0, base), lsize, white, &mut tris);
                }
                let mut x = x0 + label_w;
                for (d, dw) in b.data.iter().zip(&data_w) {
                    let t = fit(d, *dw, dsize);
                    let tx = if b.data_right { x + dw - text::width(&t, dsize) - 3.0 * s } else { x };
                    text::label(&t, DVec2::new(tx, base), dsize, white, &mut tris);
                    x += dw;
                }
            }
        }
        // -showGrid: the sections' columns and the small blocks' rows.
        if let Some(ci) = crate::hud::grid() {
            let c = aztek_scene::draw_style::INDEX_COLORS[ci.clamp(0, 31) as usize];
            let c = [c[0], c[1], c[2], 255];
            for k in 1..5 {
                let x = col_w * k as f64;
                seg(DVec2::new(x, 0.0), DVec2::new(x, h), c);
            }
            let mut y = 0.0;
            while y <= h * 0.5 {
                seg(DVec2::new(0.0, y), DVec2::new(w, y), c);
                seg(DVec2::new(0.0, h - y), DVec2::new(w, h - y), c);
                y += 20.0 * s;
            }
        }
        // The built-in rows, under the section's blocks.
        let size = 10.0 * s;
        let row_h = 15.0 * s;
        for section in [0i64, 1, 2, 3, 4, 5, 6, 7, 8, 9] {
            let col: Vec<&String> = hud.iter().filter(|(k, _)| *k == section).map(|(_, t)| t).collect();
            let skip = used_px[section as usize];
            for (i, t) in col.iter().enumerate() {
                let tw = text::width(t, size);
                let x = match section % 5 {
                    0 | 1 => 10.0 * s,
                    2 => w * 0.5 - tw * 0.5,
                    _ => w - tw - 10.0 * s,
                };
                let x = if section == 4 || section == 9 { w - tw - 10.0 * s } else { x };
                let y = if section < 5 { skip + 16.0 * s + i as f64 * row_h } else { h - skip - 26.0 * s - (col.len() - 1 - i) as f64 * row_h };
                text::label(t, DVec2::new(x, y), size, c8([230, 230, 230, 255]), &mut tris);
            }
        }
        // What scripts put there: buttons as boxes, sliders as a track and a handle, drawn in lines like the rest.
        for p in &hud_parts {
            let (white, dim) = ([230, 230, 230, 255], [150, 150, 150, 255]);
            text::label(&p.text, p.text_at, p.text_size, c8(white), &mut tris);
            if p.slider {
                let mid = (p.lo.y + p.hi.y) * 0.5;
                seg(DVec2::new(p.lo.x, mid), DVec2::new(p.hi.x, mid), dim);
                let t = if p.max > p.min { ((p.value - p.min) / (p.max - p.min)).clamp(0.0, 1.0) } else { 0.0 };
                let x = p.lo.x + (p.hi.x - p.lo.x) * t;
                let r = 3.0 * s;
                let (a, b, c, d) = (DVec2::new(x - r, p.lo.y), DVec2::new(x + r, p.lo.y), DVec2::new(x + r, p.hi.y), DVec2::new(x - r, p.hi.y));
                seg(a, b, white);
                seg(b, c, white);
                seg(c, d, white);
                seg(d, a, white);
                let value = if p.int { format!("{}", p.value.round() as i64) } else { format!("{:.*}", p.precision, p.value) };
                text::label(&value, DVec2::new(p.hi.x + 6.0 * s, p.text_at.y), p.value_size, c8(white), &mut tris);
            } else {
                let (a, b, c, d) = (p.lo, DVec2::new(p.hi.x, p.lo.y), p.hi, DVec2::new(p.lo.x, p.hi.y));
                seg(a, b, white);
                seg(b, c, white);
                seg(c, d, white);
                seg(d, a, white);
            }
        }
        // Grid numbers.
        let screen = self.vp.screen();
        for (text, at) in self.vp.grid_labels() {
            if let Some(p) = screen.project(at) {
                text::label(&text, p + DVec2::new(3.0 * s, -3.0 * s), size, c8([200, 200, 200, 255]), &mut tris);
            }
        }
        // Dimension and annotation labels.
        for id in core.s.graph.nodes() {
            let Some(n) = core.s.graph.node(id) else { continue };
            let Some((text, at)) = aztek_scene::rig_misc::shape_label(n) else { continue };
            let Some(p) = screen.project(at) else { continue };
            text::label(&text, p + DVec2::new(6.0 * s, -6.0 * s), size, c8([230, 230, 230, 255]), &mut tris);
        }
        // The Normal orientation names its handles.
        if self.vp.axis_mode == "normal" && self.vp.tool == Tool::Move {
            if let Some(pv) = self.vp.manip_pivot(&mut core.s, items) {
                let len = self.vp.handle_len(pv, s);
                let screen = self.vp.screen();
                for (i, label) in ["U", "V", "N"].into_iter().enumerate() {
                    if let Some(at) = screen.project(pv + self.vp.frame.col(i) * len * 1.1) {
                        text::label(label, at, size, c8([255, 255, 255, 255]), &mut tris);
                    }
                }
            }
        }
        // What the drag has done so far, by the cursor.
        if let Some((at, t)) = &readout {
            text::label(t, *at + DVec2::new(14.0 * s, -10.0 * s), size, c8([255, 240, 150, 255]), &mut tris);
        }
        if lines.is_empty() && tris.is_empty() {
            return;
        }
        // Pixels straight to clip space.
        let m = DMat4::from_cols_array(&[2.0 / w, 0.0, 0.0, 0.0, 0.0, 2.0 / h, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, -1.0, 0.0, 1.0]);
        if !tris.is_empty() {
            let b = self.gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &tris);
            self.r.set_viewport(&self.gpu, 0.0, 0.0, w as f32, h as f32);
            self.r.bind(&self.gpu, Kind::ColorTrisOnTop);
            let mut p: Push = bytemuck::Zeroable::zeroed();
            p.mvp = m.as_mat4().to_cols_array();
            p.n1[3] = 1.0;
            p.color = [1.0; 4];
            self.r.push(&self.gpu, &p);
            self.r.draw(&self.gpu, &b, None, tris.len() as u32);
            self.gpu.drop_buffer(b);
        }
        if lines.is_empty() {
            return;
        }
        let b = self.gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &lines);
        self.r.set_viewport(&self.gpu, 0.0, 0.0, w as f32, h as f32);
        self.r.bind(&self.gpu, Kind::LinesOnTop);
        self.r.set_line_width(&self.gpu, (1.0 * s) as f32);
        let mut p: Push = bytemuck::Zeroable::zeroed();
        p.mvp = m.as_mat4().to_cols_array();
        p.n1[3] = 1.0;
        p.color = [1.0; 4];
        self.r.push(&self.gpu, &p);
        self.r.draw(&self.gpu, &b, None, lines.len() as u32);
        self.gpu.drop_buffer(b);
    }

    /// The drag's offset in numbers, as the reference shows in the view during a manipulator drag.
    fn readout(&self) -> Option<(DVec2, String)> {
        let d = self.vp.drag.as_ref()?;
        if !d.applied || !crate::ui_cmds::pref_bool("manipReadout", true) {
            return None;
        }
        let (_, call) = self.last_drag.as_ref()?;
        let by = call.get("by").or_else(|| call.get("uvn"))?.as_array()?;
        // In the current units, as the channel box shows them.
        let name = self.last_drag.as_ref().map(|(n, _)| n.as_str()).unwrap_or("");
        let k = if name.starts_with("rotate") {
            1.0 / aztek_ops::units::rad_per_unit()
        } else if name.starts_with("move") || name.starts_with("polyMove") {
            1.0 / aztek_ops::units::cm_per_unit()
        } else {
            1.0
        };
        let t: Vec<String> = by.iter().filter_map(Value::as_f64).map(|x| format!("{:.3}", x * k)).collect();
        Some((self.vp.screen().project(d.pivot)?, t.join(" ")))
    }

    /// Film gate and resolution aspect ratios of the view's camera.
    fn gate_aspects(&self, core: &mut Core) -> (f64, f64) {
        use aztek_scene::cameras as cam;
        let g = &core.s.graph;
        let mut film = 1.5;
        if let Ok(Some(id)) = g.find(&self.cam_node) {
            if let Some(sh) = cam::shape_of(g, id) {
                let d = g.node(sh).unwrap().data();
                film = d.float(cam::H_APERTURE) / d.float(cam::V_APERTURE).max(1e-6);
            }
        }
        (film, crate::ui_cmds::pref_f64("resolutionAspect", 960.0 / 540.0))
    }

    /// Remembers the camera before a move, for [.
    fn cam_push(&mut self) {
        if self.cam_undo.len() > 64 {
            self.cam_undo.remove(0);
        }
        self.cam_undo.push(self.vp.cam.clone());
        self.cam_redo.clear();
    }

    /// Selects what's under the brush or cursor, adding to what the drag has picked.
    fn drag_select_at(&mut self, core: &mut Core, items: &[crate::viewport::DrawItem], px: DVec2, mods: i32) {
        let picked = if self.vp.tool == Tool::PaintSelect {
            let r = crate::ui_cmds::pref_f64("paintSelectRadius", 22.0) * self.dpr;
            self.vp.select_brush = Some((px, r));
            self.vp.pick_inside(&core.s, items, &|p: DVec2| (p - px).length() <= r)
        } else {
            self.vp.pick(&core.s, items, px)
        };
        if picked.is_empty() {
            return;
        }
        let mut key = if mods & CTRL != 0 && mods & SHIFT == 0 { "deselect" } else { "add" };
        // The paint skin weights tool's Paint Select: add, remove or toggle, as its setting says.
        if self.skin_select && mods & (CTRL | SHIFT) == 0 {
            key = match core.s.paint_ctx.paint_select_mode {
                2 => "deselect",
                3 => "toggle",
                _ => "add",
            };
        }
        let _ = core.exec_silent("select", json!({"args": picked, key: true}));
    }

    /// Records a pick for Shift+double-click's path.
    fn note_pick(&mut self, picked: &[String]) {
        let Some(p) = picked.first() else { return };
        let Some((obj, rest)) = p.split_once('.') else { return };
        let Some((kind, id)) = rest.split_once('[') else { return };
        let Ok(id) = id.trim_end_matches(']').parse::<u32>() else { return };
        self.pick_hist.push((obj.to_string(), kind.to_string(), id));
        if self.pick_hist.len() > 2 {
            self.pick_hist.remove(0);
        }
    }

    // -----------------------------------------------------------------------------------------------------------------
    // Mouse

    /// Follows the camera node when a script moved it.
    fn read_camera(&mut self, core: &mut Core) {
        use aztek_scene::cameras as cam;
        let g = &mut core.s.graph;
        let Ok(Some(id)) = g.find(&self.cam_node) else { return };
        let light = aztek_scene::lights::shape_of(g, id);
        let Some(sh) = cam::shape_of(g, id).or(light) else { return };
        g.evaluate();
        let tr = aztek_scene::dag::transform_of(g, sh);
        let w = aztek_scene::rig::world(g, tr);
        let eye = w.w_axis.truncate();
        let z = w.z_axis.truncate().normalize_or_zero();
        let d = g.node(sh).unwrap().data();
        // Through a light: a spot light's cone is the view, the rest a default lens.
        let (hfov, ow, coi, ortho) = if light.is_some() {
            let n = g.node(sh).unwrap();
            let read = |name: &str, fallback: f64| n.ty().attr_index(name).and_then(|i| n.value(i).scalar()).unwrap_or(fallback);
            (read("coneAngle", 54.43).clamp(1.0, 170.0), self.vp.cam.ortho_width, read("centerOfIllumination", 5.0).max(1e-3), false)
        } else {
            let fl = d.float(cam::FOCAL_LENGTH);
            let hfa = d.float(cam::H_APERTURE);
            ((2.0 * (hfa * 25.4 * 0.5 / fl.max(1e-6)).atan()).to_degrees(), d.float(cam::ORTHO_WIDTH), d.float(cam::COI).max(1e-3), d.bool(cam::ORTHOGRAPHIC))
        };
        let (yaw, pitch) = crate::camera::Camera::angles_of(z);
        // The camera's turn about its view direction against the one yaw and pitch give: a camera that faces the
        // world's up axis still shows what its own up says (the ortho views after upAxis without -rv).
        let roll = {
            let mut probe = self.vp.cam.clone();
            probe.yaw = yaw;
            probe.pitch = pitch;
            probe.roll = 0.0;
            probe.coi = eye - probe.dir_to_eye() * coi;
            probe.distance = coi;
            let up = probe.view().transform_vector3(w.y_axis.truncate().normalize_or_zero());
            let r = 90.0 - up.y.atan2(up.x).to_degrees();
            let r = (r + 180.0).rem_euclid(360.0) - 180.0;
            if r.abs() < 1e-6 { 0.0 } else { r }
        };
        // The film's 2D pan and zoom (panZoom), a camera's only.
        let pan_zoom = if light.is_none() {
            let n = g.node(sh).unwrap();
            let read = |name: &str, fallback: f64| n.ty().attr_index(name).and_then(|i| n.value(i).scalar()).unwrap_or(fallback);
            (read("panZoomEnabled", 0.0) != 0.0).then(|| [read("horizontalPan", 0.0), read("verticalPan", 0.0), read("zoom", 1.0), d.float(cam::H_APERTURE)])
        } else {
            None
        };
        let pz = pan_zoom.unwrap_or([0.0, 0.0, 1.0, 0.0]);
        let state = [eye.x, eye.y, eye.z, yaw, pitch, coi, ow, hfov, ortho as i32 as f64, roll, pz[0], pz[1], pz[2], pz[3]];
        let same = self.cam_sync.is_some_and(|s| s.iter().zip(&state).all(|(a, b)| (a - b).abs() < 1e-6));
        if same {
            return;
        }
        self.cam_sync = Some(state);
        let c = &mut self.vp.cam;
        c.yaw = yaw;
        c.pitch = pitch;
        c.roll = roll;
        c.distance = coi;
        c.ortho_width = ow;
        c.hfov = hfov;
        c.orthographic = ortho;
        c.pan_zoom = pan_zoom;
        c.coi = eye - c.dir_to_eye() * coi;
    }

    /// Writes what the mouse did to the view back to the camera node. Not undoable, as the reference's default.
    pub fn write_camera(&mut self, core: &mut Core) {
        use aztek_scene::cameras as cam;
        use aztek_scene::plug::{PlugRef, PlugValue};
        use aztek_scene::transform::Transform;
        let g = &mut core.s.graph;
        let Ok(Some(id)) = g.find(&self.cam_node) else { return };
        let light = aztek_scene::lights::shape_of(g, id);
        let Some(sh) = cam::shape_of(g, id).or(light) else { return };
        let tr = aztek_scene::dag::transform_of(g, sh);
        let (eye, yaw, pitch, dist, ow, ortho) = {
            let c = &self.vp.cam;
            (c.eye(), c.yaw, c.pitch, c.distance, c.ortho_width, c.orthographic)
        };
        // A rolled camera keeps its roll: its rotate comes from the whole view.
        let rotate = if self.vp.cam.roll != 0.0 {
            let m = glam::DMat3::from_mat4(self.vp.cam.view().inverse());
            let (rz, ry, rx) = m.to_euler(glam::EulerRot::ZYX);
            glam::DVec3::new(rx, ry, rz)
        } else {
            cam::world_rotation(cam::rotate_for(yaw.to_radians(), pitch.to_radians()))
        };
        // Driven or locked cameras keep to what drives them.
        g.without_undo(|g| {
            let _ = g.set_value(PlugRef::new(tr, Transform::TRANSLATE as i32), PlugValue::Vec3(eye));
            let _ = g.set_value(PlugRef::new(tr, Transform::ROTATE as i32), PlugValue::Vec3(rotate));
            if light.is_some() {
                if let Some(i) = g.node(sh).and_then(|n| n.ty().attr_index("centerOfIllumination")) {
                    let _ = g.set_value(PlugRef::new(sh, i as i32), PlugValue::Float(dist));
                }
            } else {
                let _ = g.set_value(PlugRef::new(sh, cam::COI as i32), PlugValue::Float(dist));
            }
            if ortho && light.is_none() {
                let _ = g.set_value(PlugRef::new(sh, cam::ORTHO_WIDTH as i32), PlugValue::Float(ow));
            }
        });
        // What's written is what's read back, so the next read leaves the view alone.
        self.cam_sync = None;
        self.read_camera(core);
    }

    /// kind: 0 press, 1 move, 2 release. button: 1 left, 2 middle, 3 right (0 on moves). buttons: held, bit per button.
    /// Returns whether the view should redraw.
    pub fn mouse(&mut self, core: &mut Core, kind: i32, button: i32, px: DVec2, mods: i32, buttons: i32) -> bool {
        // The paint skin weights tool's Select and Paint Select modes pick as the select and paint select tools do.
        let mode = core.s.paint_ctx.skin_paint_mode;
        if self.vp.tool == Tool::PaintSkin && mode != 1 {
            self.vp.tool = if mode == 0 { Tool::Select } else { Tool::PaintSelect };
            self.skin_select = true;
            let r = self.mouse_with_tool(core, kind, button, px, mods, buttons);
            self.skin_select = false;
            self.vp.tool = Tool::PaintSkin;
            return r;
        }
        self.mouse_with_tool(core, kind, button, px, mods, buttons)
    }

    /// What a modelling tool works on: everything but the live surface, which only Quad Draw and Create Polygon draw
    /// onto and nothing edits (#127).
    fn tool_items(&self, core: &Core, items: &[crate::viewport::DrawItem]) -> Vec<crate::viewport::DrawItem> {
        let live = &core.s.live_surface;
        items
            .iter()
            .filter(|i| live.is_empty() || matches!(self.vp.tool, Tool::QuadDraw | Tool::CreatePolygon) || !live.contains(&i.shape))
            .map(|i| crate::viewport::DrawItem { shape: i.shape, xf: i.xf, world: i.world, mesh: i.mesh.clone(), smooth: i.smooth })
            .collect()
    }

    fn mouse_with_tool(&mut self, core: &mut Core, kind: i32, button: i32, px: DVec2, mods: i32, buttons: i32) -> bool {
        self.iso_in(core);
        self.vp.click_box = crate::ui_cmds::pref_f64("clickBoxSize", 4.0).max(1.0) * self.dpr;
        let gathered = gather(&mut core.s.graph);
        let mut items = self.vp.filter_items(&core.s.graph, gathered);
        let alt = mods & ALT != 0;
        if !alt && self.hud_mouse(core, kind, button, px) {
            return true;
        }
        if !alt {
            if let Some(r) = self.cube_mouse(core, kind, button, px, buttons) {
                return r;
            }
        }
        if !alt && self.nav.is_none() && self.vp.tool == Tool::ShowManip && !self.brush_adjust {
            if let Some(r) = crate::node_manip::mouse(core, &self.vp, kind, button, px, buttons, self.dpr) {
                return r;
            }
        }
        if !alt && self.nav.is_none() && !self.brush_adjust && (kind != 1 || buttons == 0 || buttons & 1 != 0 || buttons & 2 != 0) {
            let tool_items = self.tool_items(core, &items);
            if let Some(r) = self.tool_mouse(core, kind, button, px, mods, &tool_items) {
                return r;
            }
        }
        match kind {
            0 => {
                if alt {
                    self.cam_push();
                    self.nav = Some((button, px));
                    return false;
                }
                // The camera tools: the left or middle button drives them.
                let cam_nav = match self.vp.tool {
                    Tool::CamTumble => Some(1),
                    Tool::CamTrack => Some(2),
                    Tool::CamDolly => Some(3),
                    Tool::CamZoom => Some(4),
                    Tool::CamRoll => Some(5),
                    Tool::CamYawPitch => Some(6),
                    Tool::CamWalk => Some(6),
                    Tool::ModelTime => Some(8),
                    _ => None,
                };
                if self.vp.tool == Tool::CamWalk && button == 2 {
                    self.cam_push();
                    self.nav = Some((7, px));
                    return false;
                }
                if self.vp.tool == Tool::ModelTime && button == 1 {
                    self.time_drag = aztek_scene::anim::frame(&core.s.graph);
                }
                if let Some(b) = cam_nav {
                    if button == 1 {
                        self.cam_push();
                        self.nav = Some((b, px));
                    }
                    return false;
                }
                if button == 1 {
                    match self.vp.tool {
                        Tool::CamBoxDolly => {
                            self.marquee = Some(px);
                            return true;
                        }
                        Tool::CurveBezier => {
                            let at = self.vp.place_point(px);
                            self.bezier.push((at, at));
                            self.bezier_drag = true;
                            self.bezier_preview();
                            return true;
                        }
                        Tool::CreateCube | Tool::CreatePlane | Tool::CreateRound => {
                            let at = self.vp.place_point(px);
                            match self.prim {
                                // The height's click finishes the cube.
                                Some(p) if p.stage == 2 => self.prim_finish(core, p),
                                _ => {
                                    self.prim = Some(PrimDraft { a: at, b: at, height: 0.0, stage: 1, press_y: px.y });
                                    self.prim_preview();
                                }
                            }
                            return true;
                        }
                        Tool::MergeEdge | Tool::MergeFacet => {
                            let picked = self.vp.pick(&core.s, &items, px);
                            self.note_pick(&picked);
                            select_picked(core, picked, SHIFT | CTRL);
                            if self.pick_hist.len() == 2 && self.pick_hist[0].0 == self.pick_hist[1].0 && self.pick_hist[0].1 == self.pick_hist[1].1 && self.pick_hist[0].2 != self.pick_hist[1].2 {
                                let (a, b) = (self.pick_hist[0].clone(), self.pick_hist[1].clone());
                                let call = if self.vp.tool == Tool::MergeEdge {
                                    ("polyMergeEdge", json!({"args": [a.0], "firstEdge": a.2, "secondEdge": b.2}))
                                } else {
                                    ("polyMergeFacet", json!({"args": [a.0], "firstFacet": a.2, "secondFacet": b.2}))
                                };
                                let _ = core.exec(call.0, call.1);
                                self.pick_hist.clear();
                            }
                            return true;
                        }
                        Tool::SnapTogether => {
                            if let Some((i, _, world)) = self.vp.raycast(&items, px) {
                                let obj = core.s.graph.path(items[i].xf);
                                match self.snap_first.take() {
                                    None => {
                                        self.snap_first = Some((obj, world));
                                        self.vp.tool_points = vec![(world, [255, 255, 60, 255])];
                                    }
                                    Some((first, from)) => {
                                        let by = world - from;
                                        let _ = core.exec("move", json!({"args": [first], "by": [by.x, by.y, by.z]}));
                                        self.vp.tool_points.clear();
                                    }
                                }
                            }
                            return true;
                        }
                        _ => {}
                    }
                }
                // B held: the drag sizes soft select's falloff.
                if self.brush_adjust && (button == 1 || button == 2) && !matches!(self.vp.tool, Tool::Sculpt | Tool::PaintSkin | Tool::Groom) {
                    self.soft_drag = Some((px, aztek_scene::soft_select::state().1));
                    self.soft_cursor = Some(px);
                    self.brush_dragged = false;
                    return true;
                }
                if self.vp.tool == Tool::Groom && button == 1 {
                    if self.brush_adjust {
                        let size = aztek_scene::xjen::groom::ctx().lock().unwrap().size;
                        self.brush_drag = Some((px, size));
                        self.brush_dragged = true;
                        return true;
                    }
                    if let Some((i, wp, n)) = self.vp.brush_hit(&core.s, &items, px) {
                        let mesh = items[i].shape;
                        let descs = aztek_scene::xjen::brush_descriptions(&core.s.graph);
                        if let Some(d) = descs.into_iter().find(|&d| aztek_scene::xjen::bound_mesh(&core.s.graph, d) == Some(mesh)) {
                            self.groom = aztek_scene::xjen::groom::Stroke::begin(&mut core.s, d, mods & CTRL != 0, mods & SHIFT != 0);
                            self.groom_dab(core, wp, n);
                            self.vp.brush = Some((wp, n));
                        }
                    }
                    return true;
                }
                if self.vp.tool == Tool::Sculpt && button == 1 {
                    if self.brush_adjust {
                        self.brush_drag = Some((px, core.s.sculpt_ctx.size));
                        self.brush_dragged = true;
                        return true;
                    }
                    if let Some((i, wp, n)) = self.vp.brush_hit(&core.s, &items, px) {
                        let shape = items[i].shape;
                        if let Some(m) = aztek_scene::modifiers::cage(&mut core.s.graph, shape) {
                            let pos = m.positions().to_vec();
                            self.sculpt = Some(SculptStroke { shape, base: pos.clone(), pos, mesh: m, world: items[i].world, applied: false, invert: mods & CTRL != 0, smooth: mods & SHIFT != 0, last: None });
                            self.sculpt_dab(core, wp, n);
                            self.vp.brush = Some((wp, n));
                        }
                    }
                    return true;
                }
                if self.vp.tool == Tool::PaintAttr && button == 1 {
                    if self.brush_adjust {
                        self.brush_drag = Some((px, core.s.attr_ctx.paint.radius));
                        self.brush_dragged = true;
                        return true;
                    }
                    if let Some((i, wp, n)) = self.vp.brush_hit(&core.s, &items, px) {
                        if let Some((node, slot)) = aztek_scene::deform_paint::current(&core.s, items[i].shape) {
                            let size = items[i].mesh.point_capacity() as usize;
                            let weights = aztek_scene::deform_paint::read(&core.s.graph, node, slot, size);
                            let mask = aztek_scene::deform_paint::mask(&core.s.graph, node, size);
                            self.attr_stroke = Some(AttrStroke { node, slot, weights, mask, applied: false, invert: mods & CTRL != 0, smooth: mods & SHIFT != 0, last: None });
                            self.attr_dab(core, &items, i, wp, n);
                            self.vp.brush = Some((wp, n));
                        }
                        return true;
                    }
                    self.marquee = Some(px);
                    return false;
                }
                if self.vp.tool == Tool::PaintSkin && button == 1 {
                    if self.brush_adjust {
                        self.brush_drag = Some((px, core.s.paint_ctx.radius));
                        self.brush_dragged = true;
                        return true;
                    }
                    if let Some((i, wp, n)) = self.vp.brush_hit(&core.s, &items, px) {
                        let skin = aztek_scene::skin::skin_of(&core.s.graph, items[i].shape);
                        if let Some(skin) = skin {
                            let layers = aztek_scene::skin_layers::layers_of(&core.s.graph, skin).and_then(|l| {
                                let id = l.current?;
                                let t = aztek_scene::skin_layers::paint_target(&core.s.graph, skin, &l, id)?;
                                Some((l, id, t))
                            });
                            if let (Some(b), Some(inf)) = (aztek_scene::skin::bind_of(&core.s.graph, skin), aztek_scene::skin::paint_influence(&core.s, skin)) {
                                self.stroke = Some(PaintStroke { skin, inf, bind: (*b).clone(), layers, applied: false, invert: mods & CTRL != 0, smooth: mods & SHIFT != 0, last: None });
                                self.paint_dab(core, &items, i, wp, n);
                                self.vp.brush = Some((wp, n));
                            }
                        }
                        return true;
                    }
                    self.marquee = Some(px);
                    return false;
                }
                self.vp.update_frame(&mut core.s);
                let pivot = self.vp.manip_pivot(&mut core.s, &items);
                let mut pivot = pivot;
                let mut hit = pivot.and_then(|pv| self.vp.manip_hit(pv, px, self.dpr));
                // Tweak Mode: a press off the handles on something takes it, and the drag moves it.
                if button == 1 && hit.is_none() && self.vp.tweak && self.vp.tool == Tool::Move && mods & (SHIFT | CTRL) == 0 {
                    let picked = self.vp.pick(&core.s, &items, px);
                    if !picked.is_empty() {
                        select_picked(core, picked, 0);
                        self.vp.update_frame(&mut core.s);
                        pivot = self.vp.manip_pivot(&mut core.s, &items);
                        hit = pivot.map(|_| Handle::Center);
                    }
                }
                // Ctrl-click on a handle sets the constraint. On a move or scale axis it drags as well, in the other two
                // axes, as the reference's: Ctrl on the Y scale handle scales X and Z (#86).
                if button == 1 && mods & CTRL != 0 && mods & SHIFT == 0 {
                    if let Some(h) = hit {
                        self.vp.set_constraint(h);
                        match (self.vp.eff_tool(), h) {
                            (Tool::Move | Tool::Scale, Handle::Axis(i)) if !self.vp.pivot_mode => hit = Some(Handle::Plane(i)),
                            _ => return true,
                        }
                    }
                }
                // Ctrl+middle drag with the Move tool on components: along their normals (#102).
                let along_normals = button == 2
                    && mods & CTRL != 0
                    && mods & SHIFT == 0
                    && self.vp.eff_tool() == Tool::Move
                    && !self.vp.pivot_mode
                    && components::mode(&core.s.graph) != ComponentMode::Object;
                // Shift-drag on a handle makes something new and drags that, as the reference does: selected faces or
                // edges are extruded, objects duplicated.
                if button == 1 && mods & SHIFT != 0 && mods & CTRL == 0 && hit.is_some() && pivot.is_some() && matches!(self.vp.eff_tool(), Tool::Move | Tool::Rotate | Tool::Scale) && !self.vp.pivot_mode {
                    // Each can be turned off (Shift Extrude and Shift Duplicate in the tool's options).
                    let made = match components::mode(&core.s.graph) {
                        ComponentMode::Face | ComponentMode::Edge if crate::ui_cmds::pref_bool("shiftExtrude", true) => core.exec("polyExtrude", json!({})).is_ok(),
                        ComponentMode::Object if crate::ui_cmds::pref_bool("shiftDuplicate", true) => core.exec("duplicate", json!({})).is_ok(),
                        _ => false,
                    };
                    if made {
                        let gathered = gather(&mut core.s.graph);
                        items = self.vp.filter_items(&core.s.graph, gathered);
                        self.vp.update_frame(&mut core.s);
                        pivot = self.vp.manip_pivot(&mut core.s, &items).or(pivot);
                    }
                }
                let remembered = if self.vp.pivot_mode { None } else { crate::viewport::active_handle(self.vp.eff_tool()) };
                let start_drag = match (button, hit, pivot) {
                    (2, _, Some(_)) if along_normals => Some(Handle::Normal),
                    (1, Some(h), Some(_)) => {
                        // A clicked axis, plane or centre becomes the tool's active handle, as in the reference (#128).
                        if !self.vp.pivot_mode && matches!(h, Handle::Axis(_) | Handle::Plane(_) | Handle::Center | Handle::ViewRing | Handle::Trackball) {
                            crate::viewport::set_active_handle(self.vp.eff_tool(), Some(h));
                        }
                        Some(h)
                    }
                    // Middle drag anywhere drags the active handle, or in the view plane.
                    (2, h, Some(_)) => Some(h.or(remembered).or(self.vp.hover).or(self.vp.constraint).unwrap_or(match self.vp.eff_tool() {
                        Tool::Rotate => Handle::ViewRing,
                        _ => Handle::Center,
                    })),
                    _ => None,
                };
                if let (Some(h), Some(pv)) = (start_drag, pivot) {
                    let comps = components::mode(&core.s.graph) != ComponentMode::Object;
                    let targets = core.s.graph.selection().iter().map(|&id| core.s.graph.path(id)).collect();
                    // Points a V-snapped move can land on: other objects' vertices, or the unselected ones of these.
                    let mut snap_points = Vec::new();
                    {
                        let g = &core.s.graph;
                        let sel = g.selection();
                        let mode = components::mode(g);
                        for it in &items {
                            let picked = sel.contains(&it.xf);
                            // An object's own points are places for its pivot to go, but not for itself.
                            if picked && !comps && !self.vp.pivot_mode {
                                continue;
                            }
                            let skip: std::collections::HashSet<u32> = if picked {
                                components::convert(&it.mesh, mode, components::selected(g, it.shape, mode), ComponentMode::Vertex, false).into_iter().collect()
                            } else {
                                Default::default()
                            };
                            for p in 0..it.mesh.point_capacity() {
                                if it.mesh.point_valid(p) && !skip.contains(&p) {
                                    snap_points.push(it.world.transform_point3(it.mesh.position(p)));
                                }
                            }
                        }
                    }
                    let snap_curves = crate::viewport::snap_curves(&core.s.graph);
                    let choose_axis = button == 2 && mods & SHIFT != 0 && hit.is_none();
                    let lead = core.s.graph.selection().last().copied();
                    let (start_rotate, start_scale) = match lead.and_then(|l| crate::viewport::channels(&core.s.graph, l)) {
                        Some((r, sc)) => (Some(r), Some(sc)),
                        None => (None, None),
                    };
                    self.vp.drag = Some(Drag { handle: h, start: px, pivot: pv, applied: false, components: comps, choose_axis, targets, snap_points, snap_curves, start_rotate, start_scale });
                    self.drag_lines = 0;
                    return true;
                }
                if button == 1 && crate::ui_cmds::selection_locked() && matches!(self.vp.tool, Tool::Select | Tool::Move | Tool::Rotate | Tool::Scale | Tool::Lasso | Tool::PaintSelect) {
                    return false;
                }
                if button == 1 && self.vp.tool == Tool::Lasso {
                    self.lassoing = true;
                    self.vp.lasso = vec![px];
                    return true;
                }
                if button == 1 && (self.vp.tool == Tool::PaintSelect || self.tab_held) {
                    self.drag_selecting = true;
                    if mods & (SHIFT | CTRL) == 0 && !self.skin_select {
                        select_picked(core, Vec::new(), 0);
                    }
                    self.drag_select_at(core, &items, px, mods);
                    return true;
                }
                if button == 1 && self.vp.tool == Tool::ShortestPath {
                    let picked = self.vp.pick(&core.s, &items, px);
                    self.note_pick(&picked);
                    if self.pick_hist.len() == 2 && self.pick_hist[0].0 == self.pick_hist[1].0 && self.pick_hist[0].1 == self.pick_hist[1].1 {
                        let (a, b) = (self.pick_hist[0].clone(), self.pick_hist[1].clone());
                        let kind = match a.1.as_str() {
                            "vtx" => "vertex",
                            "e" => "edge",
                            _ => "face",
                        };
                        let _ = core.exec("selectPathBetween", json!({"args": [a.0], "from": a.2, "to": b.2, "kind": kind}));
                    } else {
                        select_picked(core, picked, mods | SHIFT | CTRL);
                    }
                    return true;
                }
                if button == 1 && matches!(self.vp.tool, Tool::Joint | Tool::CurveCv | Tool::CurveEp | Tool::CurveAddPt | Tool::ArcTwo | Tool::ArcThree) {
                    self.joint_press = Some(px);
                    if self.vp.tool == Tool::Joint {
                        self.joint_curves = crate::viewport::snap_curves(&core.s.graph);
                    }
                    return false;
                }
                if button == 1 {
                    self.marquee = Some(px);
                }
                false
            }
            1 => {
                // The pointer leaving the view is reported as a move far away.
                if px.x < -1e5 && self.nav.is_some() {
                    return false;
                }
                // The joint tool: the joint follows the pointer until the button comes up.
                if self.joint_press.is_some() && self.vp.tool == Tool::Joint && px.x > -1e5 {
                    let g = &core.s.graph;
                    let from = g.selection().last().copied().filter(|&id| aztek_scene::rig::is_joint(g, id)).and_then(|id| g.node(id)).map(|n| n.data().matrix(aztek_scene::transform::DagNode::WORLD_MATRIX).w_axis.truncate());
                    self.vp.joint_preview = Some((from, self.vp.place_joint_point(&self.joint_curves, px)));
                    return true;
                }
                if let Some((b, last)) = self.nav {
                    let d = px - last;
                    let aspect = self.vp.aspect();
                    match b {
                        1 => self.vp.cam.tumble(d / self.dpr),
                        2 => self.vp.cam.track(d, self.vp.rect[3], aspect),
                        // Zoom: the lens changes, the camera stays.
                        4 => {
                            if self.vp.cam.ortho() {
                                self.vp.cam.ortho_width = (self.vp.cam.ortho_width * (1.0 - d.x * 0.003)).max(1e-3);
                            } else {
                                self.vp.cam.hfov = (self.vp.cam.hfov - d.x * 0.1).clamp(2.0, 170.0);
                            }
                        }
                        // Roll: about the view axis.
                        5 => self.vp.cam.roll += d.x * 0.3,
                        // Yaw-Pitch: the camera turns where it stands, its centre of interest moves.
                        6 => {
                            let c = &mut self.vp.cam;
                            let eye = c.eye();
                            c.yaw -= d.x * 0.2;
                            c.pitch = (c.pitch + d.y * 0.2).clamp(-89.9, 89.9);
                            c.coi = eye - c.dir_to_eye() * c.distance;
                        }
                        // Walk: forward and back on the view's line, sideways by tracking.
                        7 => {
                            let k = self.vp.cam.distance * 0.005;
                            let dir = self.vp.cam.dir_to_eye();
                            self.vp.cam.coi += dir * d.y * k;
                            self.vp.cam.track(DVec2::new(d.x, 0.0), self.vp.rect[3], aspect);
                        }
                        8 => {
                            self.time_drag += d.x / self.dpr * 0.1;
                            let _ = core.exec_quiet("currentTime", json!({"args": [self.time_drag.round()]}), aztek_ops::CallSource::Script);
                            self.nav = Some((b, px));
                            return true;
                        }
                        _ => self.vp.cam.dolly((d.x + d.y) / self.dpr),
                    }
                    self.nav = Some((b, px));
                    self.write_camera(core);
                    return true;
                }
                if self.bezier_drag {
                    let at = self.vp.place_point(px);
                    if let Some(last) = self.bezier.last_mut() {
                        last.1 = at;
                    }
                    self.bezier_preview();
                    return true;
                }
                if let Some(mut p) = self.prim {
                    let at = self.vp.place_point(px);
                    match p.stage {
                        1 if buttons != 0 => p.b = at,
                        2 => {
                            let upp = self.vp.cam.units_per_px_at(p.a, self.vp.rect[3], self.vp.aspect());
                            p.height = (p.press_y - px.y) * upp;
                        }
                        _ => {}
                    }
                    self.prim = Some(p);
                    self.prim_preview();
                    return true;
                }
                if self.brush_adjust && self.soft_drag.is_none() && !matches!(self.vp.tool, Tool::Sculpt | Tool::PaintSkin | Tool::PaintAttr | Tool::Groom) {
                    self.soft_cursor = Some(px);
                    if buttons == 0 {
                        return true;
                    }
                }
                if let Some((start, radius)) = self.soft_drag {
                    let upp = self.vp.cam.units_per_px_at(self.vp.cam.coi, self.vp.rect[3], self.vp.aspect());
                    let next = (radius + (px.x - start.x) * upp).max(1e-3);
                    self.brush_dragged = true;
                    let _ = core.exec_silent("softSelect", json!({"softSelectDistance": next}));
                    return true;
                }
                if self.lassoing {
                    if self.vp.lasso.last().is_none_or(|l| (*l - px).length() > 2.0 * self.dpr) {
                        self.vp.lasso.push(px);
                    }
                    return true;
                }
                if self.drag_selecting {
                    self.drag_select_at(core, &items, px, mods);
                    return true;
                }
                if self.vp.tool == Tool::PaintSelect && buttons == 0 {
                    self.vp.select_brush = Some((px, crate::ui_cmds::pref_f64("paintSelectRadius", 22.0) * self.dpr));
                    return true;
                }
                if let (Tool::Groom, Some((start, size))) = (self.vp.tool, self.brush_drag) {
                    // B drag: the brush's size on screen.
                    aztek_scene::xjen::groom::ctx().lock().unwrap().size = (size + (px.x - start.x)).max(1.0);
                    core.version += 1;
                    return true;
                }
                if let Some((start, radius)) = self.brush_drag {
                    let upp = self.vp.cam.units_per_px_at(self.vp.brush.map(|b| b.0).unwrap_or(self.vp.cam.coi), self.vp.rect[3], self.vp.aspect());
                    let next = (radius + (px.x - start.x) * upp).max(1e-3);
                    if self.vp.tool == Tool::Sculpt {
                        core.s.sculpt_ctx.size = next;
                        core.version += 1;
                    } else if self.vp.tool == Tool::PaintAttr {
                        core.s.attr_ctx.paint.radius = next;
                        core.version += 1;
                    } else {
                        let _ = core.exec_silent("artAttrSkinPaintCtx", json!({"edit": true, "radius": next}));
                    }
                    return true;
                }
                if self.groom.is_some() {
                    if let Some((_, wp, n)) = self.vp.brush_hit(&core.s, &items, px) {
                        self.groom_dab(core, wp, n);
                        self.vp.brush = Some((wp, n));
                    } else if let Some((c, n)) = self.vp.brush {
                        // Off the surface (Grab and Comb follow on), on the plane through the last point.
                        let (o, d) = self.vp.cam.ray(px, self.vp.size());
                        let den = d.dot(n);
                        if den.abs() > 1e-6 {
                            let wp = o + d * ((c - o).dot(n) / den);
                            self.groom_dab(core, wp, n);
                        }
                    }
                    return true;
                }
                if self.vp.tool == Tool::Groom && buttons == 0 {
                    let was = self.vp.brush.is_some();
                    self.vp.brush = self.vp.brush_hit(&core.s, &items, px).map(|x| (x.1, x.2));
                    return was || self.vp.brush.is_some();
                }
                if self.sculpt.is_some() {
                    if let Some((_, wp, n)) = self.vp.brush_hit(&core.s, &items, px) {
                        self.sculpt_dab(core, wp, n);
                        self.vp.brush = Some((wp, n));
                    } else if core.s.sculpt_ctx.tool == "grab" {
                        // Grab keeps following off the surface, on the plane through the stroke's last point.
                        if let (Some(l), Some((_, n))) = (self.sculpt.as_ref().and_then(|s| s.last), self.vp.brush) {
                            let (o, d) = self.vp.cam.ray(px, self.vp.size());
                            let den = d.dot(n);
                            if den.abs() > 1e-6 {
                                let t = (l - o).dot(n) / den;
                                let wp = o + d * t;
                                self.sculpt_dab(core, wp, n);
                                self.vp.brush = Some((wp, n));
                            }
                        }
                    }
                    return true;
                }
                if self.vp.tool == Tool::Sculpt && buttons == 0 {
                    let was = self.vp.brush.is_some();
                    self.vp.brush = self.vp.brush_hit(&core.s, &items, px).map(|x| (x.1, x.2));
                    return was || self.vp.brush.is_some();
                }
                if self.stroke.is_some() {
                    if let Some((i, wp, n)) = self.vp.brush_hit(&core.s, &items, px) {
                        self.paint_dab(core, &items, i, wp, n);
                        self.vp.brush = Some((wp, n));
                    }
                    return true;
                }
                if self.attr_stroke.is_some() {
                    if let Some((i, wp, n)) = self.vp.brush_hit(&core.s, &items, px) {
                        self.attr_dab(core, &items, i, wp, n);
                        self.vp.brush = Some((wp, n));
                    }
                    return true;
                }
                if matches!(self.vp.tool, Tool::PaintSkin | Tool::PaintAttr) && buttons == 0 {
                    let was = self.vp.brush.is_some();
                    self.vp.brush = self.vp.brush_hit(&core.s, &items, px).map(|x| (x.1, x.2));
                    return was || self.vp.brush.is_some();
                }
                self.vp.live_auto = !core.s.live_surface.is_empty();
                if (self.vp.snap == 5 || (self.vp.snap == 0 && self.vp.live_auto)) && self.vp.drag.is_some() {
                    let live: Vec<crate::viewport::DrawItem> = items
                        .iter()
                        .filter(|it| core.s.live_surface.contains(&it.shape))
                        .map(|it| crate::viewport::DrawItem { shape: it.shape, xf: it.xf, world: it.world, mesh: it.mesh.clone(), smooth: 0 })
                        .collect();
                    self.vp.live_hit = self.vp.raycast(&live, px).map(|h| h.2);
                }
                if let Some(d) = self.vp.drag.as_mut().filter(|d| d.choose_axis) {
                    if (px - d.start).length() < 4.0 * self.dpr {
                        return false;
                    }
                    d.choose_axis = false;
                    let (pivot, delta) = (d.pivot, px - d.start);
                    let h = self.vp.axis_along(pivot, delta);
                    self.vp.drag.as_mut().unwrap().handle = h;
                }
                if let Some(d) = &self.vp.drag {
                    if px == d.start {
                        return false;
                    }
                    if let Some((cmd, call)) = self.vp.drag_call(d, px, self.dpr) {
                        // Previews run quietly; the release echoes the one command that stays.
                        if d.applied {
                            core.retract_preview();
                        }
                        let ok = core.exec_silent(&cmd, call.clone()).is_ok();
                        self.last_drag = if ok { Some((cmd, call)) } else { None };
                        self.vp.drag.as_mut().unwrap().applied = ok;
                    }
                    return true;
                }
                if let Some(start) = self.marquee {
                    self.vp.marquee = Some((start, px));
                    return true;
                }
                if buttons == 0 {
                    let pivot = self.vp.manip_pivot(&mut core.s, &items);
                    let hover = pivot.and_then(|pv| self.vp.manip_hit(pv, px, self.dpr));
                    let comp = if hover.is_none() {
                        self.vp.pick_component(&core.s, &items, px).map(|(i, k, id)| (items[i].shape, k, id))
                    } else {
                        None
                    };
                    if hover != self.vp.hover || comp != self.vp.hover_comp {
                        self.vp.hover = hover;
                        self.vp.hover_comp = comp;
                        return true;
                    }
                }
                false
            }
            3 => {
                if button != 1 {
                    return false;
                }
                // Shift+double-click: the loop or ring through the last two picks, or the shorter way between them.
                if mods & SHIFT != 0 && self.pick_hist.len() == 2 {
                    let (a, b) = (self.pick_hist[0].clone(), self.pick_hist[1].clone());
                    if a.0 == b.0 && a.1 == b.1 && a.2 != b.2 {
                        let kind = match a.1.as_str() {
                            "vtx" => "vertex",
                            "e" => "edge",
                            _ => "face",
                        };
                        let _ = core.exec("selectPathBetween", json!({"args": [a.0], "from": a.2, "to": b.2, "kind": kind}));
                        return true;
                    }
                }
                // Double-click: an edge loop in edge mode, as the reference does.
                if components::mode(&core.s.graph) == ComponentMode::Edge {
                    let _ = core.exec("selectLoop", json!({}));
                    return true;
                }
                // In face mode the face's whole shell, its polygon island (#154); Shift adds it to what's picked.
                if components::mode(&core.s.graph) == ComponentMode::Face {
                    if let Some((obj, kind, id)) = self.pick_hist.last().cloned() {
                        if kind == "f" {
                            let mut call = json!({"args": [obj], "id": id, "kind": "face"});
                            if mods & SHIFT != 0 {
                                call["add"] = json!(true);
                            }
                            let _ = core.exec("selectConnected", call);
                            return true;
                        }
                    }
                }
                false
            }
            _ => {
                // A camera tool's left drag carries another number than the button it is on.
                let tool_drag = button == 1 && matches!(self.vp.tool, Tool::CamTumble | Tool::CamTrack | Tool::CamDolly | Tool::CamZoom | Tool::CamRoll | Tool::CamYawPitch | Tool::CamWalk | Tool::ModelTime);
                if self.nav.is_some_and(|(b, _)| b == button || tool_drag || (b == 7 && button == 2)) {
                    self.nav = None;
                    return true;
                }
                if self.bezier_drag {
                    self.bezier_drag = false;
                    return true;
                }
                if let Some(mut p) = self.prim.filter(|p| p.stage == 1) {
                    p.press_y = px.y;
                    let dragged = (p.b - p.a).length() > 1e-6;
                    let tall = matches!(self.vp.round_kind.as_str(), "polyCylinder" | "polyCone");
                    if (self.vp.tool == Tool::CreateCube || (self.vp.tool == Tool::CreateRound && tall)) && dragged {
                        p.stage = 2;
                        self.prim = Some(p);
                    } else {
                        self.prim_finish(core, p);
                    }
                    return true;
                }
                if let Some((start, _)) = self.soft_drag.take() {
                    if (px - start).length() > 1.0 {
                        let r = aztek_scene::soft_select::state().1;
                        let _ = core.exec("softSelect", json!({"softSelectDistance": r}));
                    }
                    return true;
                }
                if self.lassoing {
                    self.lassoing = false;
                    let outline = std::mem::take(&mut self.vp.lasso);
                    let picked = if outline.len() < 3 {
                        self.vp.pick(&core.s, &items, px)
                    } else {
                        self.vp.pick_lasso(&core.s, &items, &outline)
                    };
                    select_picked(core, picked, mods);
                    return true;
                }
                if self.drag_selecting {
                    self.drag_selecting = false;
                    self.vp.select_brush = None;
                    core.version += 1;
                    return true;
                }
                if self.brush_drag.take().is_some() {
                    return true;
                }
                if self.sculpt.take().is_some() {
                    core.version += 1;
                    return true;
                }
                if self.groom.take().is_some() {
                    core.version += 1;
                    return true;
                }
                if self.attr_stroke.take().is_some() {
                    core.version += 1;
                    return true;
                }
                if self.stroke.take().is_some() {
                    // The dabs' one undo entry stays; the panels hear of it.
                    core.version += 1;
                    return true;
                }
                if let Some(d) = self.vp.drag.take() {
                    if d.applied {
                        if let Some((cmd, call)) = self.last_drag.take() {
                            core.retract_preview();
                            let _ = core.exec(&cmd, call);
                            // Snap Pivot Orientation: a pivot moved onto a vertex, edge or face takes its axes.
                            if self.vp.pivot_mode && self.vp.snap_pivot_ori && !matches!(d.handle, Handle::PivotRing(_)) && !d.targets.is_empty() {
                                let _ = core.exec("setPivot", json!({"args": d.targets, "orientToSurface": true}));
                            }
                            // A pivot turned in the view is what the tools then follow: their axes go to Object, which
                            // shows the pivot's. The reference goes to its Custom orientation here.
                            if matches!(d.handle, Handle::PivotRing(_)) {
                                for (kind, name) in [("manipMoveContext", "Move"), ("manipRotateContext", "Rotate"), ("manipScaleContext", "Scale")] {
                                    let _ = core.exec_silent(kind, json!({"edit": true, "args": [name], "mode": 0}));
                                }
                            }
                        }
                    }
                    return true;
                }
                if let Some(start) = self.joint_press.take() {
                    // A joint goes where the button comes up, however far it was dragged.
                    if self.vp.tool == Tool::Joint {
                        self.vp.joint_preview = None;
                        let at = self.vp.place_joint_point(&std::mem::take(&mut self.joint_curves), px);
                        place_joint(core, at);
                    } else if (px - start).length() < 6.0 * self.dpr {
                        let at = self.vp.place_point(px);
                        {
                            self.vp.curve_points.push(at);
                            let want = match self.vp.tool {
                                Tool::ArcTwo => 2,
                                Tool::ArcThree => 3,
                                _ => 0,
                            };
                            if want > 0 && self.vp.curve_points.len() == want {
                                let flat: Vec<[f64; 3]> = self.vp.curve_points.drain(..).map(|p| p.to_array()).collect();
                                let cmd = if want == 2 { "twoPointArc" } else { "threePointArc" };
                                let mut call = json!({"point1": flat[0], "point2": flat[1]});
                                if want == 3 {
                                    call["point3"] = json!(flat[2]);
                                }
                                let _ = core.exec(cmd, call);
                            }
                        }
                    }
                    return true;
                }
                if self.vp.tool == Tool::CamBoxDolly {
                    if let Some(start) = self.marquee.take() {
                        self.vp.marquee = None;
                        self.box_dolly(core, start, px);
                        return true;
                    }
                }
                if let Some(start) = self.marquee.take() {
                    self.vp.marquee = None;
                    if crate::ui_cmds::selection_locked() {
                        return true;
                    }
                    let picked = if (px - start).length() < 4.0 * self.dpr {
                        // Too short for a marquee: a click over the little box dragged, as wide as the click box at least.
                        let k = self.vp.click_box;
                        let half = (px - start).abs() * 0.5;
                        self.vp.click_box = k + half.max_element();
                        let p = self.vp.pick(&core.s, &items, (start + px) * 0.5);
                        self.vp.click_box = k;
                        self.note_pick(&p);
                        p
                    } else {
                        self.vp.pick_rect(&core.s, &items, start, px)
                    };
                    select_picked(core, picked, mods);
                    return true;
                }
                false
            }
        }
    }


    /// The box in the view fills it: dragged left to right the camera comes in, the other way it goes out.
    fn box_dolly(&mut self, core: &mut Core, start: DVec2, end: DVec2) {
        if (end - start).length() < 4.0 * self.dpr {
            return;
        }
        let [_, _, w, h] = self.vp.rect;
        let middle = (start + end) * 0.5;
        let aspect = self.vp.aspect();
        self.cam_push();
        self.vp.cam.track(DVec2::new(w * 0.5, h * 0.5) - middle, h, aspect);
        let ratio = ((end.x - start.x).abs() / w.max(1.0)).max((end.y - start.y).abs() / h.max(1.0)).clamp(1e-3, 1.0);
        let scale = if end.x >= start.x { ratio } else { 1.0 / ratio };
        if self.vp.cam.ortho() {
            self.vp.cam.ortho_width = (self.vp.cam.ortho_width * scale).max(1e-3);
        } else {
            self.vp.cam.distance = (self.vp.cam.distance * scale).max(1e-3);
        }
        self.write_camera(core);
        self.needs_redraw = true;
    }

    /// Anchors, tangents and the curve so far, as the tool's lines.
    fn bezier_preview(&mut self) {
        let cvs = bezier_cvs(&self.bezier);
        self.vp.tool_lines.clear();
        self.vp.tool_points = self.bezier.iter().map(|x| (x.0, [255, 255, 255, 255])).collect();
        for (a, out) in &self.bezier {
            let mirrored = *a * 2.0 - *out;
            self.vp.tool_lines.push((mirrored, *out, [150, 150, 150, 255]));
        }
        for at in (0..cvs.len().saturating_sub(3)).step_by(3) {
            let (p0, p1, p2, p3) = (cvs[at], cvs[at + 1], cvs[at + 2], cvs[at + 3]);
            let mut last = p0;
            for k in 1..=24 {
                let t = k as f64 / 24.0;
                let u = 1.0 - t;
                let q = p0 * (u * u * u) + p1 * (3.0 * u * u * t) + p2 * (3.0 * u * t * t) + p3 * (t * t * t);
                self.vp.tool_lines.push((last, q, [70, 255, 120, 255]));
                last = q;
            }
        }
    }

    /// The draft's outline: the base, and the height for a cube.
    fn prim_preview(&mut self) {
        self.vp.tool_lines.clear();
        let Some(p) = self.prim else { return };
        let (c, size) = prim_box(&p);
        let (x, z) = (size.x * 0.5, size.z * 0.5);
        let corners = [DVec3::new(-x, 0.0, -z), DVec3::new(x, 0.0, -z), DVec3::new(x, 0.0, z), DVec3::new(-x, 0.0, z)].map(aztek_scene::world::to_world);
        let up = aztek_scene::world::up() * p.height;
        let green = [70, 255, 120, 255];
        for i in 0..4 {
            let (a, b) = (c + corners[i], c + corners[(i + 1) % 4]);
            self.vp.tool_lines.push((a, b, green));
            if p.height.abs() > 1e-9 {
                self.vp.tool_lines.push((a + up, b + up, green));
                self.vp.tool_lines.push((a, a + up, green));
            }
        }
    }

    fn prim_finish(&mut self, core: &mut Core, p: PrimDraft) {
        self.prim = None;
        self.vp.tool_lines.clear();
        let (c, size) = prim_box(&p);
        // A click with no drag: the unit size, where it was clicked.
        let (w, d) = if size.x < 1e-6 || size.z < 1e-6 { (1.0, 1.0) } else { (size.x, size.z) };
        if self.vp.tool == Tool::CreateRound {
            let kind = self.vp.round_kind.clone();
            let radius = if w == 1.0 && d == 1.0 && size.x < 1e-6 { 1.0 } else { (w.max(d) * 0.5).max(1e-3) };
            let call = match kind.as_str() {
                "polyCylinder" | "polyCone" => json!({"radius": radius, "height": if p.height.abs() < 1e-6 { radius * 2.0 } else { p.height.abs() }}),
                "polyTorus" => json!({"radius": radius, "sectionRadius": radius * 0.25}),
                _ => json!({"radius": radius}),
            };
            if core.exec(&kind, call).is_ok() {
                let at = if p.height.abs() > 1e-6 { c + aztek_scene::world::up() * p.height * 0.5 } else { c };
                let _ = core.exec("move", json!({"to": [at.x, at.y, at.z]}));
            }
            return;
        }
        let cube = self.vp.tool == Tool::CreateCube;
        let height = if p.height.abs() < 1e-6 { w.min(d) } else { p.height.abs() };
        let made = if cube { core.exec("polyCube", json!({"width": w, "height": height, "depth": d})) } else { core.exec("polyPlane", json!({"width": w, "height": d})) };
        if made.is_ok() {
            // A cube stands on its base, or hangs from it when the height was dragged down.
            let at = if cube { c + aztek_scene::world::up() * height * 0.5 * if p.height < 0.0 { -1.0 } else { 1.0 } } else { c };
            let _ = core.exec("move", json!({"to": [at.x, at.y, at.z]}));
        }
    }

    fn groom_dab(&mut self, core: &mut Core, wp: DVec3, n: DVec3) {
        let size = aztek_scene::xjen::groom::ctx().lock().unwrap().size;
        let radius = size * self.vp.cam.units_per_px_at(wp, self.vp.rect[3], self.vp.aspect());
        if let Some(st) = self.groom.as_mut() {
            st.dab(&mut core.s, wp, n, radius);
            core.version += 1;
        }
    }

    fn sculpt_dab(&mut self, core: &mut Core, wp: DVec3, n: DVec3) {
        let Some(st) = self.sculpt.as_mut() else { return };
        let mut ctx = core.s.sculpt_ctx.clone();
        ctx.mirror = aztek_scene::mesh_nodes::symmetry_spec(&core.s.graph, st.shape).filter(|s| !s.is_radial());
        if ctx.tool == "freeze" {
            let mut fr = core.s.sculpt_ctx.frozen.remove(&st.shape).unwrap_or_default();
            aztek_scene::sculpt::paint_freeze(&mut fr, &st.mesh, &st.world, wp, &ctx, st.invert);
            core.s.sculpt_ctx.frozen.insert(st.shape, fr);
            return;
        }
        let pull = st.last.map(|l| wp - l).unwrap_or(DVec3::ZERO);
        let frozen = ctx.frozen.get(&st.shape).map(|v| v.as_slice());
        aztek_scene::sculpt::dab(&mut st.pos, &st.base, &st.mesh, &st.world, wp, n, pull, &ctx, frozen, st.invert, st.smooth);
        st.last = Some(wp);
        if st.applied {
            core.s.undo();
        }
        let (shape, pos, mesh) = (st.shape, st.pos.clone(), st.mesh.clone());
        let ok = aztek_scene::sculpt::commit(&mut core.s, shape, &pos, &mesh).is_ok();
        if let Some(st) = self.sculpt.as_mut() {
            st.applied = ok;
        }
        core.version += 1;
    }

    /// A dab of the paint attribute brush, as paint_dab does for skin weights.
    fn attr_dab(&mut self, core: &mut Core, items: &[crate::viewport::DrawItem], i: usize, wp: DVec3, n: DVec3) {
        let Some(st) = self.attr_stroke.as_mut() else { return };
        let ctx = core.s.attr_ctx.paint.clone();
        let dabs: Vec<DVec3> = match st.last {
            Some(l) => {
                let steps = (((wp - l).length() / (ctx.radius * 0.25).max(1e-6)).ceil() as usize).clamp(1, 32);
                (1..=steps).map(|k| l.lerp(wp, k as f64 / steps as f64)).collect()
            }
            None => vec![wp],
        };
        for at in dabs {
            aztek_scene::deform_paint::stamp(&mut st.weights, st.mask.as_deref(), &items[i].mesh, &items[i].world, at, Some(n), &ctx, st.invert, st.smooth);
        }
        st.last = Some(wp);
        if st.applied {
            core.s.undo();
        }
        st.applied = aztek_scene::deform_paint::write(&mut core.s.graph, st.node, st.slot, st.weights.clone()).is_ok();
        core.version += 1;
    }

    /// A dab of the brush, replacing the stroke's previous undo entry so the whole stroke undoes in one step.
    fn paint_dab(&mut self, core: &mut Core, items: &[crate::viewport::DrawItem], i: usize, wp: DVec3, n: DVec3) {
        let Some(st) = self.stroke.as_mut() else { return };
        let ctx = core.s.paint_ctx.clone();
        // Dabs spaced by a fraction of the radius, so a fast drag leaves no gaps.
        let dabs = match st.last {
            Some(l) => {
                let steps = (((wp - l).length() / (ctx.radius * 0.25).max(1e-6)).ceil() as usize).clamp(1, 32);
                (1..=steps).map(|k| l.lerp(wp, k as f64 / steps as f64)).collect()
            }
            None => vec![wp],
        };
        let brush = aztek_scene::skin_layers::brush();
        for at in dabs {
            match st.layers.as_mut() {
                Some((l, id, t)) => {
                    aztek_scene::skin_layers::stamp_layer(l, *id, *t, &items[i].mesh, &items[i].world, at, Some(n), &ctx, &brush, st.invert, st.smooth);
                }
                None => {
                    aztek_scene::skin::stamp(&mut st.bind, &items[i].mesh, &items[i].world, at, Some(n), st.inf, &ctx, st.invert, st.smooth);
                }
            }
        }
        st.last = Some(wp);
        if st.applied {
            core.s.undo();
        }
        st.applied = match st.layers.as_ref() {
            Some((l, id, t)) => {
                let mut l = l.clone();
                if brush.mirror {
                    let what = [matches!(t, aztek_scene::skin_layers::Target::Influence(_)), *t == aztek_scene::skin_layers::Target::Mask, *t == aztek_scene::skin_layers::Target::Dq];
                    let _ = aztek_scene::skin_layers::mirror(&mut core.s.graph, st.skin, &mut l, *id, what);
                }
                aztek_scene::skin_layers::commit(&mut core.s.graph, st.skin, l).is_ok()
            }
            None => aztek_scene::skin::commit_paint(&mut core.s.graph, st.skin, st.bind.clone()).is_ok(),
        };
        core.version += 1;
    }

    pub fn wheel(&mut self, core: &mut Core, delta: f64) {
        self.vp.cam.dolly(delta * 0.5);
        self.write_camera(core);
    }

    pub fn set(&mut self, core: &mut Core, key: &str, v: &Value) {
        match key {
            "tool" => {
                self.ts = Default::default();
                self.bezier.clear();
                self.bezier_drag = false;
                self.prim = None;
                self.snap_first = None;
                self.vp.tool_lines.clear();
                self.vp.tool_points.clear();
                self.vp.constraint = None;
                self.vp.select_brush = None;
                self.pick_hist.clear();
                self.vp.tool = match v.as_str().unwrap_or("") {
                    "showManip" => Tool::ShowManip,
                    "lasso" => Tool::Lasso,
                    "paintSelect" => Tool::PaintSelect,
                    "shortestPath" => Tool::ShortestPath,
                    "camTumble" => Tool::CamTumble,
                    "camTrack" => Tool::CamTrack,
                    "camDolly" => Tool::CamDolly,
                    "camZoom" => Tool::CamZoom,
                    "camRoll" => Tool::CamRoll,
                    "camYawPitch" => Tool::CamYawPitch,
                    "move" => Tool::Move,
                    "rotate" => Tool::Rotate,
                    "scale" => Tool::Scale,
                    "joint" => Tool::Joint,
                    "paintSkin" => Tool::PaintSkin,
                    "paintAttr" => Tool::PaintAttr,
                    "sculpt" => Tool::Sculpt,
                    "groom" => Tool::Groom,
                    "insertJoint" => Tool::InsertJoint,
                    "ikHandle" => Tool::IkHandle,
                    "ikSplineHandle" => Tool::IkSplineHandle,
                    "curveCv" => Tool::CurveCv,
                    "curveEp" => Tool::CurveEp,
                    "multiCut" => Tool::MultiCut,
                    "insertLoop" => Tool::InsertLoop,
                    "offsetLoop" => Tool::OffsetLoop,
                    "targetWeld" => Tool::TargetWeld,
                    "slideEdge" => Tool::SlideEdge,
                    "crease" => Tool::Crease,
                    "createPolygon" => Tool::CreatePolygon,
                    "appendPolygon" => Tool::AppendPolygon,
                    "quadDraw" => Tool::QuadDraw,
                    "cutSewUv" => Tool::CutSew,
                    "camBoxDolly" => Tool::CamBoxDolly,
                    "camWalk" => Tool::CamWalk,
                    "modelTime" => Tool::ModelTime,
                    "curveAddPt" => Tool::CurveAddPt,
                    "curveBezier" => Tool::CurveBezier,
                    "arcTwo" => Tool::ArcTwo,
                    "arcThree" => Tool::ArcThree,
                    "mergeEdge" => Tool::MergeEdge,
                    "mergeFacet" => Tool::MergeFacet,
                    "createCube" => Tool::CreateCube,
                    "createPlane" => Tool::CreatePlane,
                    "createSphere" | "createCylinder" | "createCone" | "createTorus" => {
                        self.vp.round_kind = format!("poly{}", &v.as_str().unwrap_or("")[6..]);
                        Tool::CreateRound
                    }
                    "snapTogether" => Tool::SnapTogether,
                    _ => Tool::Select,
                }
            }
            "wireframe" => self.vp.wireframe = v.as_bool().unwrap_or(false),
            "wireOnShaded" => self.vp.wire_on_shaded = v.as_bool().unwrap_or(false),
            "renderer" => {
                if let Some(k) = v.as_str().and_then(RendererKind::parse) {
                    self.renderer = k;
                }
            }
            "grid" => self.show_grid = v.as_bool().unwrap_or(true),
            "camera" => {
                if let Some(name) = v.as_str() {
                    if let Some(k) = CamKind::from_name(name) {
                        self.vp.cam = crate::camera::Camera::new(k);
                        self.vp.reset_grid(&mut self.gpu);
                    }
                    self.cam_node = name.to_string();
                    self.cam_sync = None;
                    self.read_camera(core);
                }
            }
            "frame" => {
                let items = gather(&mut core.s.graph);
                let items = self.vp.filter_items(&core.s.graph, items);
                self.cam_push();
                self.vp.frame(&mut core.s, &items, v.as_str() == Some("all"));
                self.write_camera(core);
            }
            "viewUndo" => {
                if let Some(c) = self.cam_undo.pop() {
                    self.cam_redo.push(self.vp.cam.clone());
                    self.vp.cam = c;
                    self.write_camera(core);
                }
            }
            "viewRedo" => {
                if let Some(c) = self.cam_redo.pop() {
                    self.cam_undo.push(self.vp.cam.clone());
                    self.vp.cam = c;
                    self.write_camera(core);
                }
            }
            "isolate" => {
                use aztek_scene::isolation as iso;
                let on = |k: &str| v.get(k).and_then(Value::as_bool).unwrap_or(false);
                if let Some(state) = v.get("state").and_then(Value::as_bool) {
                    if state {
                        self.vp.iso = iso::from_selection(&mut core.s.graph);
                    } else {
                        self.vp.iso.clear();
                    }
                } else if on("addSelected") {
                    iso::add_selection(&mut self.vp.iso, &mut core.s.graph);
                } else if on("removeSelected") {
                    iso::remove_selection(&mut self.vp.iso, &mut core.s.graph);
                } else if on("update") {
                    self.vp.iso = iso::from_selection(&mut core.s.graph);
                } else if self.vp.iso.active() {
                    self.vp.iso.clear();
                } else {
                    self.vp.iso = iso::from_selection(&mut core.s.graph);
                }
                self.iso_in(core);
                core.version += 1;
            }
            "disp" => {
                let d = &mut self.vp.disp;
                if let Some(o) = v.as_object() {
                    for (k, x) in o {
                        let b = x.as_bool().unwrap_or(false);
                        match k.as_str() {
                            "displayLights" => {
                                if let Some(m) = x.as_str().and_then(crate::lighting::LightMode::parse) {
                                    d.light_mode = m;
                                }
                            }
                            "lights" => d.lights = b,
                            "twoSided" => d.two_sided = b,
                            "shadows" => d.shadows = b,
                            "bounceLight" => d.bounce_light = b,
                            "depthOfField" => d.depth_of_field = b,
                            "ambientOcclusion" => d.ambient_occlusion = b,
                            "aoRadius" => d.ao_radius = x.as_f64().unwrap_or(2.0).clamp(0.01, 1000.0),
                            "aoIntensity" => d.ao_intensity = x.as_f64().unwrap_or(1.5).clamp(0.0, 8.0),
                            "polygons" => d.polygons = b,
                            "surfaces" => d.surfaces = b,
                            "curves" => d.curves = b,
                            "joints" => d.joints = b,
                            "ikHandles" => d.ik_handles = b,
                            "deformers" => d.deformers = b,
                            "locators" => d.locators = b,
                            "cameras" => d.cameras = b,
                            "manipulators" => d.manipulators = b,
                            "flat" => d.flat = b,
                            "textured" => d.textured = b,
                            "defaultMaterial" => d.default_material = b,
                            "boundingBox" => d.bounding_box = b,
                            "xray" => d.xray = b,
                            "xrayJoints" => d.xray_joints = b,
                            "xrayActive" => d.xray_active = b,
                            "backface" => d.backface = b,
                            "filmGate" => d.film_gate = b,
                            "resGate" => d.res_gate = b,
                            "safeAction" => d.safe_action = b,
                            "highlight" => d.highlight = b,
                            "borderEdges" => d.border_edges = b,
                            "faceNormals" => d.face_normals = b,
                            "vertexNormals" => d.vertex_normals = b,
                            "hardEdges" => d.hard_edges = b,
                            "creaseEdges" => d.crease_edges = b,
                            "limitToSelected" => d.limit_to_selected = b,
                            "normalSize" => d.normal_size = x.as_f64().unwrap_or(d.normal_size),
                            _ => {}
                        }
                    }
                }
            }
            "camState" => {
                let c = &mut self.vp.cam;
                let g = |k: &str, d: f64| v.get(k).and_then(Value::as_f64).unwrap_or(d);
                c.yaw = g("yaw", c.yaw);
                c.pitch = g("pitch", c.pitch);
                c.distance = g("distance", c.distance);
                c.ortho_width = g("orthoWidth", c.ortho_width);
                c.hfov = g("hfov", c.hfov);
                c.roll = g("roll", c.roll);
                if let Some(a) = v.get("coi").and_then(Value::as_array) {
                    if a.len() == 3 {
                        c.coi = DVec3::new(a[0].as_f64().unwrap_or(0.0), a[1].as_f64().unwrap_or(0.0), a[2].as_f64().unwrap_or(0.0));
                    }
                }
                self.write_camera(core);
            }
            "stepMove" => self.vp.step_move = v.as_f64().unwrap_or(0.0),
            "stepRotate" => self.vp.step_rotate = v.as_f64().unwrap_or(0.0),
            "stepScale" => self.vp.step_scale = v.as_f64().unwrap_or(0.0),
            "snapRelative" => self.vp.snap_relative = v.as_bool().unwrap_or(true),
            "tweakMode" => self.vp.tweak = v.as_bool().unwrap_or(false),
            "keepSpacing" => self.vp.keep_spacing = v.as_bool().unwrap_or(true),
            "snapPivot" => self.vp.snap_pivot = v.as_bool().unwrap_or(false),
            "snapPivotOri" => self.vp.snap_pivot_ori = v.as_bool().unwrap_or(false),
            "useManipPivot" => self.vp.use_manip_pivot = v.as_bool().unwrap_or(false),
            "preserveChildren" => self.vp.preserve_children = v.as_bool().unwrap_or(false),
            "preventNegativeScale" => self.vp.prevent_negative = v.as_bool().unwrap_or(true),
            "customAxis" => {
                let e = |i: usize| v.get(i).and_then(Value::as_f64).unwrap_or(0.0);
                self.vp.custom_frame = crate::axis_orient::frame_from_radians([e(0), e(1), e(2)]);
            }
            "discreteHold" => self.vp.discrete_hold = v.as_bool().unwrap_or(false),
            "tabHeld" => self.tab_held = v.as_bool().unwrap_or(false),
            "gridSpacing" => {
                self.vp.grid_spacing = v.as_f64().unwrap_or(1.0).max(1e-4);
                self.vp.reset_grid(&mut self.gpu);
            }
            "gridStyle" => {
                let st = crate::viewport::GridStyle::from_json(v);
                if st != self.vp.grid_style {
                    self.vp.grid_style = st;
                    self.vp.reset_grid(&mut self.gpu);
                }
            }
            "gridSize" => {
                self.vp.grid_extent = v.as_f64().unwrap_or(12.0).max(1.0);
                self.vp.reset_grid(&mut self.gpu);
            }
            "viewCubeVisible" => crate::view_cube::set_hidden(!v.as_bool().unwrap_or(true)),
            "viewCube" => {
                let action = v.get("action").and_then(Value::as_str).unwrap_or("redraw").to_string();
                self.cube_action(core, &action);
            }
            "home" => {
                let k = self.vp.cam.kind;
                self.vp.cam = crate::camera::Camera::new(k);
                self.write_camera(core);
            }
            "dpr" => self.dpr = v.as_f64().unwrap_or(1.0),
            "background" => {
                let g = v.as_f64().unwrap_or(0.36) as f32;
                self.background = [g, g, g, 1.0];
            }
            "parity" => self.parity = v.as_bool().unwrap_or(false),
            "manipSize" => self.vp.manip_scale = v.as_f64().unwrap_or(1.0),
            "axisOrientation" => self.vp.axis_mode = v.as_str().unwrap_or("world").to_string(),
            "curveDegree" => self.vp.curve_degree = v.as_i64().unwrap_or(3),
            "toolTab" => self.ts.tab = v.as_bool().unwrap_or(false),
            // Quad Draw's and Multi-Cut's options from the Modeling Toolkit, and its buttons (#140).
            "quadDraw" => self.ts.qd.set(&v),
            "multiCut" => {
                self.ts.mc.set(&v);
                self.vp.force_cage = self.ts.mc.force_cage;
            }
            "liveConstraint" => self.vp.live.set(&v),
            "quadDrawAction" => {
                if v.as_str() == Some("clearDots") {
                    self.ts.dots.clear();
                    self.vp.tool_points.clear();
                }
            }
            "multiCutAction" => {
                if let Some(plane) = v.as_str() {
                    self.slice_along(core, plane);
                }
            }
            // Enter: makes the curve; the tool stays for the next.
            "finishTool" => {
                if self.vp.tool == Tool::Joint {
                    finish_joint_chain(core);
                    return;
                }
                let items = gather(&mut core.s.graph);
                let items = self.tool_items(core, &items);
                self.tool_finish(core, &items);
                if self.vp.tool == Tool::CurveBezier {
                    let cvs = bezier_cvs(&self.bezier);
                    self.bezier.clear();
                    self.vp.tool_lines.clear();
                    self.vp.tool_points.clear();
                    if cvs.len() >= 4 {
                        let pts: Vec<[f64; 3]> = cvs.iter().map(|p| p.to_array()).collect();
                        let _ = core.exec("curve", json!({"bezier": true, "degree": 3, "point": pts}));
                    }
                } else if self.vp.tool == Tool::CurveAddPt {
                    let pts: Vec<[f64; 3]> = self.vp.curve_points.drain(..).map(|p| p.to_array()).collect();
                    let target = core.s.graph.selection().last().map(|&i| core.s.graph.path(i));
                    if let (false, Some(t)) = (pts.is_empty(), target) {
                        let _ = core.exec("curve", json!({"args": [t], "append": true, "point": pts}));
                    }
                } else if matches!(self.vp.tool, Tool::CurveCv | Tool::CurveEp) && self.vp.curve_points.len() >= 2 {
                    let pts: Vec<[f64; 3]> = self.vp.curve_points.drain(..).map(|p| p.to_array()).collect();
                    let flag = if self.vp.tool == Tool::CurveEp { "editPoint" } else { "point" };
                    let _ = core.exec("curve", json!({"degree": self.vp.curve_degree, flag: pts}));
                } else {
                    self.vp.curve_points.clear();
                }
            }
            "redraw" => {}
            "undoToolPoint" => {
                self.tool_back();
                self.vp.curve_points.pop();
                if self.vp.tool == Tool::CurveBezier {
                    self.bezier.pop();
                    self.bezier_preview();
                }
            }
            "brushAdjust" => {
                self.brush_adjust = v.as_bool().unwrap_or(false);
                // Each hold starts afresh, so a drag in one doesn't swallow the next tap.
                if self.brush_adjust {
                    self.brush_dragged = false;
                } else {
                    self.soft_cursor = None;
                }
            }
            "smoothCage" => self.vp.smooth_cage = v.as_bool().unwrap_or(false),
            "pivotMode" => self.vp.pivot_mode = v.as_bool().unwrap_or(false),
            "samples" => self.set_samples(v.as_u64().unwrap_or(4).clamp(1, 8) as u32),
            "snap" => {
                self.vp.snap = match v.as_str() {
                    Some("grid") => 1,
                    Some("point") => 2,
                    Some("curve") => 3,
                    Some("viewPlane") => 4,
                    Some("live") => 5,
                    _ => 0,
                }
            }
            _ => {}
        }
        self.needs_redraw = true;
    }

    pub fn get(&self, key: &str) -> Value {
        match key {
            "camera" => json!(self.cam_node),
            "tool" => json!(format!("{:?}", self.vp.tool).to_lowercase()),
            "wireframe" => json!(self.vp.wireframe),
            "wireOnShaded" => json!(self.vp.wire_on_shaded),
            "renderer" => json!(self.renderer.name()),
            "grid" => json!(self.show_grid),
            "dragging" => json!(self.vp.drag.is_some() || self.nav.is_some() || self.marquee.is_some()),
            "brushDragged" => json!(self.brush_dragged),
            "camState" => {
                let c = &self.vp.cam;
                json!({"yaw": c.yaw, "pitch": c.pitch, "distance": c.distance, "orthoWidth": c.ortho_width, "hfov": c.hfov, "roll": c.roll, "coi": [c.coi.x, c.coi.y, c.coi.z]})
            }
            "unitsPerPixel" => json!(self.vp.cam.units_per_px_at(self.vp.cam.coi, self.vp.rect[3].max(1.0), self.vp.aspect()) * self.dpr),
            "viewAxes" => {
                let m = self.vp.cam.view();
                json!({"right": [m.x_axis.x, m.y_axis.x, m.z_axis.x], "up": [m.x_axis.y, m.y_axis.y, m.z_axis.y]})
            }
            "isolated" => json!(self.vp.iso.active()),
            "disp" => {
                let d = &self.vp.disp;
                json!({"polygons": d.polygons, "surfaces": d.surfaces, "curves": d.curves, "joints": d.joints, "ikHandles": d.ik_handles, "deformers": d.deformers, "locators": d.locators, "cameras": d.cameras, "lights": d.lights, "displayLights": d.light_mode.name(), "twoSided": d.two_sided, "shadows": d.shadows, "bounceLight": d.bounce_light, "depthOfField": d.depth_of_field, "ambientOcclusion": d.ambient_occlusion, "aoRadius": d.ao_radius, "aoIntensity": d.ao_intensity, "samples": self.gpu.sample_number(), "manipulators": d.manipulators, "flat": d.flat, "textured": d.textured, "defaultMaterial": d.default_material, "boundingBox": d.bounding_box, "xray": d.xray, "xrayJoints": d.xray_joints, "xrayActive": d.xray_active, "backface": d.backface, "filmGate": d.film_gate, "resGate": d.res_gate, "safeAction": d.safe_action, "highlight": d.highlight, "grid": self.show_grid, "wireframe": self.vp.wireframe, "wireOnShaded": self.vp.wire_on_shaded})
            }
            _ => Value::Null,
        }
    }
}

thread_local! {
    /// The joints the joint tool has placed since the chain began, root first.
    static JOINT_CHAIN: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn joint_setting(key: &str) -> Value {
    crate::ctx_cmds::prop("jointContext", key)
}

/// The joint tool's click: a joint under the selected one, made with the tool's settings, the parent turned to aim at
/// it as the tool's orientation settings say, the new end joint lined up with its parent.
pub fn place_joint(core: &mut Core, at: DVec3) {
    let g = &core.s.graph;
    let parent = g.selection().last().copied().filter(|&id| aztek_scene::rig::is_joint(g, id)).map(|id| g.path(id));
    let mut call = json!({"position": [at.x, at.y, at.z]});
    let dof = joint_setting("degreeOfFreedomJ");
    if dof.as_str().is_some_and(|d| d != "xyz") {
        call["degreeOfFreedom"] = dof;
    }
    if !joint_setting("scaleCompensateJ").as_bool().unwrap_or(true) {
        call["scaleCompensate"] = json!(false);
    }
    if joint_setting("symmetry").as_bool().unwrap_or(false) {
        call["symmetry"] = json!(true);
        call["symmetryAxis"] = joint_setting("symmetryAxis");
    }
    let Ok(v) = core.exec("joint", call) else { return };
    let Some(j) = v.as_str().map(str::to_string) else { return };
    JOINT_CHAIN.with(|c| {
        let mut c = c.borrow_mut();
        if parent.is_none() {
            c.clear();
        }
        c.push(j.clone());
    });
    let Some(p) = parent else { return };
    let order = joint_setting("autoJointOrient").as_str().unwrap_or("xyz").to_string();
    let up = joint_setting("secondaryAxisOrient").as_str().unwrap_or("yup").to_string();
    let _ = core.exec_quiet("joint", json!({"args": [p], "edit": true, "orientJoint": order, "secondaryAxisOrient": up}), aztek_ops::CallSource::Script);
    // The end joint takes its parent's orientation: jointOrient zero.
    let _ = core.exec_quiet("joint", json!({"args": [j], "edit": true, "orientJoint": "none"}), aztek_ops::CallSource::Script);
}

/// Enter in the joint tool ends the chain: an IK handle down it when the tool makes one, else the selection cleared so
/// the next click starts a new chain. Nothing when no chain is being drawn.
pub fn finish_joint_chain(core: &mut Core) {
    let chain = JOINT_CHAIN.with(|c| std::mem::take(&mut *c.borrow_mut()));
    if chain.is_empty() {
        return;
    }
    if chain.len() >= 2 && joint_setting("createIKHandle").as_bool().unwrap_or(false) {
        let mut call = json!({"startJoint": chain[0], "endEffector": chain[chain.len() - 1]});
        if let Some(sol) = joint_setting("solverTypeH").as_str() {
            call["solver"] = json!(sol);
        }
        if core.exec("ikHandle", call).is_ok() {
            return;
        }
    }
    let _ = core.exec("select", json!({"clear": true}));
}

/// Component names with their mirror images, on meshes whose symmetry is on.
/// The mesh a ground plane's reflection is for: the widest flat one at the world's ground, along either up axis, and
/// that axis. Its own image would hide everything the mirrored scene puts below it (#302).
fn ground_reflector(items: &[crate::viewport::DrawItem]) -> (aztek_scene::plug::NodeId, [f32; 3], [f64; 3]) {
    let mut best = (aztek_scene::plug::NO_NODE, [0.0, 1.0, 0.0], 0.0, [0.0; 3]);
    for it in items {
        let b = it.mesh.bounds();
        let (lo, hi) = (it.world.transform_point3(b.min), it.world.transform_point3(b.max));
        // Y up and Z up are both tried: a scene keeps its own geometry whatever the preferences' up axis is.
        for (axis, up) in [(1usize, [0.0, 1.0, 0.0]), (2usize, [0.0, 0.0, 1.0])] {
            let height = (hi[axis] - lo[axis]).abs();
            if height > 0.05 || lo[axis].abs() > 0.05 || hi[axis].abs() > 0.05 {
                continue;
            }
            let (a, c) = if axis == 1 { (0, 2) } else { (0, 1) };
            let area = ((hi[a] - lo[a]) * (hi[c] - lo[c])).abs();
            if area > best.2 {
                let mut point = [(lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5, (lo.z + hi.z) * 0.5];
                point[axis] = hi[axis];
                best = (it.shape, up, area, point);
            }
        }
    }
    (best.0, best.1, best.3)
}

fn with_mirror_images(core: &mut Core, picked: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for name in picked {
        let Some((mesh, comp)) = name.rsplit_once('.') else {
            out.push(name);
            continue;
        };
        let g = &mut core.s.graph;
        let kind = match comp.split('[').next() {
            Some("vtx") => ComponentMode::Vertex,
            Some("e") => ComponentMode::Edge,
            Some("f") => ComponentMode::Face,
            _ => {
                out.push(name);
                continue;
            }
        };
        let Some(node) = g.find(mesh).ok().flatten().and_then(|id| components::as_mesh(g, id)) else {
            out.push(name);
            continue;
        };
        if aztek_scene::mesh_nodes::symmetry_spec(g, node).is_none() {
            out.push(name);
            continue;
        }
        let ids = aztek_scene::history::parse_components(comp);
        let all = components::with_symmetry(g, node, kind, &ids);
        for part in aztek_scene::history::format_components(kind, &all).split_whitespace() {
            out.push(format!("{mesh}.{part}"));
        }
    }
    out
}

/// The reference's click modifiers: Shift toggles, Ctrl deselects, Ctrl+Shift adds.
pub fn select_picked(core: &mut Core, picked: Vec<String>, mods: i32) {
    let (shift, ctrl) = (mods & SHIFT != 0, mods & CTRL != 0);
    let comps = components::mode(&core.s.graph) != ComponentMode::Object;
    let picked = if comps { with_mirror_images(core, picked) } else { picked };
    if picked.is_empty() {
        if !shift && !ctrl {
            if comps {
                // Clicking nothing in a component mode clears the components and keeps the objects hilited.
                let objs: Vec<String> = core.s.graph.selection().iter().map(|&id| core.s.graph.path(id)).collect();
                let _ = core.exec("select", json!({"args": objs}));
                components::set_all(&mut core.s.graph, Default::default(), Default::default(), Default::default());
            } else {
                let _ = core.exec("select", json!({"clear": true}));
            }
        }
        return;
    }
    // An object picked in a component mode is a click out of it: object mode, that object (#124).
    if comps && picked.iter().all(|p| !p.contains('.')) {
        let _ = core.exec("selectMode", json!({"object": true}));
    }
    let call = if ctrl && shift {
        json!({"args": picked, "add": true})
    } else if ctrl {
        json!({"args": picked, "deselect": true})
    } else if shift {
        json!({"args": picked, "toggle": true})
    } else {
        json!({"args": picked})
    };
    let _ = core.exec("select", call);
}

/// Anchors and tangent ends to a Bezier curve's points: anchor, out, in, anchor. An anchor with no tangent gets one a
/// third of the way to its neighbour.
fn bezier_cvs(anchors: &[(DVec3, DVec3)]) -> Vec<DVec3> {
    let mut out = Vec::new();
    for (i, (a, o)) in anchors.iter().enumerate() {
        if i > 0 {
            let inward = if (*o - *a).length() < 1e-9 { a.lerp(anchors[i - 1].0, 1.0 / 3.0) } else { *a * 2.0 - *o };
            out.push(inward);
        }
        out.push(*a);
        if i + 1 < anchors.len() {
            let forward = if (*o - *a).length() < 1e-9 { a.lerp(anchors[i + 1].0, 1.0 / 3.0) } else { *o };
            out.push(forward);
        }
    }
    out
}

/// A draft's base: its middle, and its size along the ground's two axes (as x and z, in the Y up frame).
fn prim_box(p: &PrimDraft) -> (DVec3, DVec3) {
    let c = (p.a + p.b) * 0.5;
    (c, aztek_scene::world::from_world(p.a - p.b).abs())
}

#[allow(dead_code)]
fn v3(v: DVec3) -> Vec3 {
    v.as_vec3()
}

/// With AZTEK_PROFILE set, how long each part of a frame took, on stderr.
struct Prof(Option<std::cell::Cell<std::time::Instant>>);

impl Prof {
    fn start() -> Prof {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        Prof(ON.get_or_init(|| std::env::var_os("AZTEK_PROFILE").is_some()).then(|| std::cell::Cell::new(std::time::Instant::now())))
    }
    fn mark(&self, what: &str) {
        if let Some(t) = &self.0 {
            eprintln!("{what} {:.2}", t.get().elapsed().as_secs_f64() * 1000.0);
            t.set(std::time::Instant::now());
        }
    }
}

#[cfg(test)]
mod renderer_tests {
    use super::RendererKind;

    #[test]
    fn the_renderer_menu_names_parse_as_the_view_expects() {
        assert_eq!(RendererKind::parse("viewport2"), Some(RendererKind::Viewport2));
        assert_eq!(RendererKind::parse("viewport 2.0"), Some(RendererKind::Viewport2));
        assert_eq!(RendererKind::parse("Realtime"), Some(RendererKind::Realtime));
        assert_eq!(RendererKind::parse("real-time"), Some(RendererKind::Realtime));
        assert_eq!(RendererKind::parse("Legacy"), Some(RendererKind::Viewport2));
        assert_eq!(RendererKind::parse("Default"), Some(RendererKind::Realtime));
        assert_eq!(RendererKind::parse("eevee"), None);
        assert_eq!(RendererKind::default(), RendererKind::Realtime);
    }
}

#[cfg(test)]
mod reflector_tests {
    use super::ground_reflector;
    use aztek_scene::plug::NO_NODE;

    #[test]
    fn the_reflections_example_has_a_ground_reflector() {
        let mut s = aztek_scene::Session::new();
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/rendering/reflections.ma");
        aztek_scene::ma_file::open_scene(&mut s, path).expect("open the reflections example");
        let items = crate::viewport::gather(&mut s.graph);
        assert_ne!(ground_reflector(&items).0, NO_NODE);
    }
}
