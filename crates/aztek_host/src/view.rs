//! One model panel: its surface, camera and mouse handling (navigation, picking, marquee, manipulator drags).

use aztek_scene::components::{self, ComponentMode};
use ash::vk;
use glam::{DMat4, DVec2, DVec3, Vec3};
use serde_json::{json, Value};

use crate::camera::CamKind;
use crate::core::Core;
use crate::font;
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
}

pub struct View {
    pub gpu: Gpu,
    pub r: Renderer,
    pub vp: Viewport,
    /// Physical pixels per logical one.
    pub dpr: f64,
    nav: Option<(i32, DVec2)>,
    marquee: Option<DVec2>,
    joint_press: Option<DVec2>,
    /// The camera node this view looks through, by name, and what was last read from or written to it.
    pub cam_node: String,
    cam_sync: Option<[f64; 9]>,
    /// The modelling tools' state.
    pub ts: crate::tools::ToolState,
    /// A sculpt stroke in progress.
    sculpt: Option<SculptStroke>,
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
    pub show_grid: bool,
    pub needs_redraw: bool,
    /// Last frame read back on request, as RGB.
    pub capture: Option<(u32, u32, Vec<u8>)>,
    pub want_capture: bool,
}

impl View {
    pub fn new(gpu: Gpu, kind: CamKind, dpr: f64) -> View {
        let r = Renderer::new(&gpu);
        View {
            gpu,
            r,
            vp: Viewport::new(kind),
            dpr,
            nav: None,
            marquee: None,
            joint_press: None,
            cam_node: kind.name().to_string(),
            cam_sync: None,
            ts: Default::default(),
            sculpt: None,
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
            show_grid: true,
            needs_redraw: true,
            capture: None,
            want_capture: false,
        }
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        if let Err(e) = self.gpu.recreate_swapchain(w, h) {
            log::error!("viewport resize: {e}");
        }
        self.needs_redraw = true;
    }

    pub fn render(&mut self, core: &mut Core) {
        let e = self.gpu.extent;
        self.vp.rect = [0.0, 0.0, e.width as f64, e.height as f64];
        let bg = self.background;
        let prof = Prof::start();
        let mut frame = self.gpu.begin_frame([bg[0], bg[1], bg[2], 1.0]);
        prof.mark("  wait for the last frame");
        if frame.is_none() {
            // Out of date (the window changed size): remake at the window's size and try once more.
            self.resize(e.width, e.height);
            frame = self.gpu.begin_frame([bg[0], bg[1], bg[2], 1.0]);
        }
        let e = self.gpu.extent;
        self.vp.rect = [0.0, 0.0, e.width as f64, e.height as f64];
        let Some(index) = frame else {
            self.needs_redraw = true;
            return;
        };
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
        self.vp.draw(&mut self.gpu, &self.r, &mut core.s, &items, self.dpr);
        prof.mark("  draw");
        if crate::usd_view::busy() {
            core.version += 1;
        }
        for w in crate::textures::with_cache(&self.gpu, |c| c.take_warnings()) {
            core.push(format!("// Warning: {w} //"), crate::core::LineKind::Warning);
        }
        self.overlay(core, &items);
        prof.mark("  overlay");
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
        for (section, _block, label, command) in crate::ui_cmds::custom_huds() {
            let lines = core.log.len();
            let val = if command.is_empty() { Value::Null } else { core.mel_checked(&command).unwrap_or(Value::Null) };
            core.log.truncate(lines);
            let text = match val {
                Value::Null => String::new(),
                Value::String(t) => t,
                o => o.to_string(),
            };
            rows.push((section, format!("{label} {text}").trim().to_string()));
        }
        rows
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
            let has_slider = kind != "hudButton";
            let has_button = kind != "hudSlider";
            let button_text = if kind == "hudButton" { label.clone() } else { txt(&c, "buttonLabel") };
            let button_w = num(&c, "buttonWidth", 0.0).max(0.0) * s;
            let button_w = if button_w > 0.0 { button_w } else { font::width(&button_text, size) + 12.0 * s };
            let track = if kind == "hudSliderButton" { num(&c, "sliderLength", 80.0) } else { num(&c, "sliderLength", 100.0) } * s;
            let label_w = if has_slider { font::width(&label, size) + 6.0 * s } else { 0.0 };
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
                out.push(HudPart { name: name.clone(), slider: true, lo: at, hi: at + DVec2::new(track, 12.0 * s), text: label.clone(), text_at: DVec2::new(x, y), min: num(&c, "minValue", 0.0), max: num(&c, "maxValue", 100.0), value: num(&c, "value", 0.0), int: txt(&c, "type") == "int", precision: num(&c, "decimalPrecision", 2.0) as usize });
                x += label_w + track + 52.0 * s;
            }
            if has_button {
                let at = DVec2::new(x, y - 7.0 * s);
                out.push(HudPart { name: name.clone(), slider: false, lo: at, hi: at + DVec2::new(button_w, 14.0 * s), text: button_text, text_at: DVec2::new(x + 6.0 * s, y), min: 0.0, max: 0.0, value: 0.0, int: false, precision: 0 });
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

    /// Screen-space extras: marquee, lasso, gates, heads up display, axis triad, camera name, numeric readout.
    fn overlay(&mut self, core: &mut Core, items: &[crate::viewport::DrawItem]) {
        let hud = self.hud_rows(core, items);
        let hud_parts = self.hud_parts();
        let readout = self.readout();
        let gate = self.gate_aspects(core);
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
        // Axis triad, bottom left.
        if crate::ui_cmds::hud_on("viewAxis") {
            let origin = DVec2::new(34.0 * s, h - 34.0 * s);
            let view = self.vp.cam.view();
            let axes = [(DVec3::X, [230, 50, 50, 255], 'x'), (DVec3::Y, [60, 220, 60, 255], 'y'), (DVec3::Z, [70, 110, 255, 255], 'z')];
            for (ax, c, name) in axes {
                let v = view.transform_vector3(ax);
                let tip = origin + DVec2::new(v.x, -v.y) * 20.0 * s;
                seg(origin, tip, c);
                let lp = origin + DVec2::new(v.x, -v.y) * 27.0 * s - DVec2::new(3.0, -4.0) * s;
                for (a, b) in font::strokes(&name.to_string(), lp, 7.0 * s) {
                    seg(a, b, c);
                }
            }
        }
        // Camera name, bottom centre.
        if crate::ui_cmds::hud_on("cameraNames") {
            let name = self.cam_node.as_str();
            let size = 10.0 * s;
            let tw = font::width(name, size);
            for (a, b) in font::strokes(name, DVec2::new(w * 0.5 - tw * 0.5, h - 12.0 * s), size) {
                seg(a, b, [225, 225, 225, 255]);
            }
        }
        // Heads up display text by section.
        let size = 9.0 * s;
        let row_h = 13.0 * s;
        for section in [0i64, 1, 2, 3, 4, 5, 6, 7, 8, 9] {
            let col: Vec<&String> = hud.iter().filter(|(k, _)| *k == section).map(|(_, t)| t).collect();
            for (i, t) in col.iter().enumerate() {
                let tw = font::width(t, size);
                let x = match section % 5 {
                    0 | 1 => 10.0 * s,
                    2 => w * 0.5 - tw * 0.5,
                    _ => w - tw - 10.0 * s,
                };
                let x = if section == 4 || section == 9 { w - tw - 10.0 * s } else { x };
                let y = if section < 5 { 16.0 * s + i as f64 * row_h } else { h - 26.0 * s - (col.len() - 1 - i) as f64 * row_h };
                for (a, b) in font::strokes(t, DVec2::new(x, y), size) {
                    seg(a, b, [230, 230, 230, 255]);
                }
            }
        }
        // What scripts put there: buttons as boxes, sliders as a track and a handle, drawn in lines like the rest.
        for p in &hud_parts {
            let (white, dim) = ([230, 230, 230, 255], [150, 150, 150, 255]);
            for (a, b) in font::strokes(&p.text, p.text_at, size) {
                seg(a, b, white);
            }
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
                for (a, b) in font::strokes(&value, DVec2::new(p.hi.x + 6.0 * s, p.text_at.y), size) {
                    seg(a, b, white);
                }
            } else {
                let (a, b, c, d) = (p.lo, DVec2::new(p.hi.x, p.lo.y), p.hi, DVec2::new(p.lo.x, p.hi.y));
                seg(a, b, white);
                seg(b, c, white);
                seg(c, d, white);
                seg(d, a, white);
            }
        }
        // Dimension and annotation labels.
        let screen = self.vp.screen();
        for id in core.s.graph.nodes() {
            let Some(n) = core.s.graph.node(id) else { continue };
            let Some((text, at)) = aztek_scene::rig_misc::shape_label(n) else { continue };
            let Some(p) = screen.project(at) else { continue };
            for (a, b) in font::strokes(&text, p + DVec2::new(6.0 * s, -6.0 * s), size) {
                seg(a, b, [230, 230, 230, 255]);
            }
        }
        // The Normal orientation names its handles.
        if self.vp.axis_mode == "normal" && self.vp.tool == Tool::Move {
            if let Some(pv) = self.vp.manip_pivot(&mut core.s, items) {
                let len = self.vp.handle_len(pv, s);
                let screen = self.vp.screen();
                for (i, label) in ["U", "V", "N"].into_iter().enumerate() {
                    if let Some(at) = screen.project(pv + self.vp.frame.col(i) * len * 1.1) {
                        for (a, b) in font::strokes(label, at, size) {
                            seg(a, b, [255, 255, 255, 255]);
                        }
                    }
                }
            }
        }
        // What the drag has done so far, by the cursor.
        if let Some((at, t)) = &readout {
            for (a, b) in font::strokes(t, *at + DVec2::new(14.0 * s, -10.0 * s), size) {
                seg(a, b, [255, 240, 150, 255]);
            }
        }
        if lines.is_empty() {
            return;
        }
        // Pixels straight to clip space.
        let m = DMat4::from_cols_array(&[2.0 / w, 0.0, 0.0, 0.0, 0.0, 2.0 / h, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, -1.0, 0.0, 1.0]);
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
        let t: Vec<String> = by.iter().filter_map(Value::as_f64).map(|x| format!("{x:.3}")).collect();
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
        let key = if mods & CTRL != 0 && mods & SHIFT == 0 { "deselect" } else { "add" };
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

    fn camera_state(&self) -> [f64; 9] {
        let c = &self.vp.cam;
        let eye = c.eye();
        [eye.x, eye.y, eye.z, c.yaw, c.pitch, c.distance, c.ortho_width, c.hfov, c.orthographic as i32 as f64]
    }

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
        let yaw = z.x.atan2(z.z).to_degrees();
        let pitch = z.y.clamp(-1.0, 1.0).asin().to_degrees();
        let state = [eye.x, eye.y, eye.z, yaw, pitch, coi, ow, hfov, ortho as i32 as f64];
        let same = self.cam_sync.is_some_and(|s| s.iter().zip(&state).all(|(a, b)| (a - b).abs() < 1e-6));
        if same {
            return;
        }
        self.cam_sync = Some(state);
        let c = &mut self.vp.cam;
        c.yaw = yaw;
        c.pitch = pitch;
        c.distance = coi;
        c.ortho_width = ow;
        c.hfov = hfov;
        c.orthographic = ortho;
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
        // Driven or locked cameras keep to what drives them.
        g.without_undo(|g| {
            let _ = g.set_value(PlugRef::new(tr, Transform::TRANSLATE as i32), PlugValue::Vec3(eye));
            let _ = g.set_value(PlugRef::new(tr, Transform::ROTATE as i32), PlugValue::Vec3(cam::rotate_for(yaw.to_radians(), pitch.to_radians())));
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
        self.iso_in(core);
        let gathered = gather(&mut core.s.graph);
        let items = self.vp.filter_items(&core.s.graph, gathered);
        let alt = mods & ALT != 0;
        if !alt && self.hud_mouse(core, kind, button, px) {
            return true;
        }
        if !alt && self.nav.is_none() && self.vp.tool == Tool::ShowManip && !self.brush_adjust {
            if let Some(r) = crate::node_manip::mouse(core, &self.vp, kind, button, px, buttons, self.dpr) {
                return r;
            }
        }
        if !alt && self.nav.is_none() && !self.brush_adjust && (kind != 1 || buttons == 0 || buttons & 1 != 0 || buttons & 2 != 0) {
            if let Some(r) = self.tool_mouse(core, kind, button, px, mods, &items) {
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
                if self.brush_adjust && (button == 1 || button == 2) && !matches!(self.vp.tool, Tool::Sculpt | Tool::PaintSkin) {
                    self.soft_drag = Some((px, aztek_scene::soft_select::state().1));
                    self.soft_cursor = Some(px);
                    self.brush_dragged = false;
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
                            if let (Some(b), Some(inf)) = (aztek_scene::skin::bind_of(&core.s.graph, skin), aztek_scene::skin::paint_influence(&core.s, skin)) {
                                self.stroke = Some(PaintStroke { skin, inf, bind: (*b).clone(), applied: false, invert: mods & CTRL != 0, smooth: mods & SHIFT != 0, last: None });
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
                // Ctrl-click on a handle sets the constraint rather than dragging.
                if button == 1 && mods & CTRL != 0 && mods & SHIFT == 0 {
                    if let Some(h) = hit {
                        self.vp.set_constraint(h);
                        return true;
                    }
                }
                let start_drag = match (button, hit, pivot) {
                    (1, Some(h), Some(_)) => Some(h),
                    // Middle drag anywhere drags the active handle, or in the view plane.
                    (2, h, Some(_)) => Some(h.or(self.vp.active_handle).or(self.vp.hover).or(self.vp.constraint).unwrap_or(match self.vp.eff_tool() {
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
                    let snap_curves: Vec<(DVec3, DVec3)> = crate::viewport::rig_items(&core.s.graph)
                        .into_iter()
                        .filter(|it| crate::viewport::type_category(&core.s.graph, it.node) == "curve")
                        .flat_map(|it| it.segs)
                        .collect();
                    let choose_axis = button == 2 && mods & SHIFT != 0 && hit.is_none();
                    self.vp.drag = Some(Drag { handle: h, start: px, pivot: pv, applied: false, components: comps, choose_axis, targets, snap_points, snap_curves });
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
                    if mods & (SHIFT | CTRL) == 0 {
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
                if self.brush_adjust && self.soft_drag.is_none() && !matches!(self.vp.tool, Tool::Sculpt | Tool::PaintSkin | Tool::PaintAttr) {
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
                if self.vp.snap == 5 && self.vp.drag.is_some() {
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
                        }
                    }
                    return true;
                }
                if let Some(start) = self.joint_press.take() {
                    if (px - start).length() < 6.0 * self.dpr {
                        let at = self.vp.place_point(px);
                        if self.vp.tool == Tool::Joint {
                            place_joint(core, at);
                        } else {
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
                        if self.vp.multi {
                            self.multi_mode(core, &items, px);
                        }
                        let p = self.vp.pick(&core.s, &items, px);
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

    /// Multi: the nearest kind under the mouse, a vertex before an edge before a face, becomes the mode.
    fn multi_mode(&mut self, core: &mut Core, items: &[crate::viewport::DrawItem], px: DVec2) {
        for kind in ["vertex", "edge", "face"] {
            let _ = core.exec_quiet("selectMode", json!({"args": [kind]}), aztek_ops::CallSource::Script);
            if self.vp.pick_component(&core.s, items, px).is_some() {
                return;
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
        let corners = [DVec3::new(-x, 0.0, -z), DVec3::new(x, 0.0, -z), DVec3::new(x, 0.0, z), DVec3::new(-x, 0.0, z)];
        let up = DVec3::Y * p.height;
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
                let y = if p.height.abs() > 1e-6 { c.y + p.height * 0.5 } else { c.y };
                let _ = core.exec("move", json!({"to": [c.x, y, c.z]}));
            }
            return;
        }
        let cube = self.vp.tool == Tool::CreateCube;
        let height = if p.height.abs() < 1e-6 { w.min(d) } else { p.height.abs() };
        let made = if cube { core.exec("polyCube", json!({"width": w, "height": height, "depth": d})) } else { core.exec("polyPlane", json!({"width": w, "height": d})) };
        if made.is_ok() {
            // A cube stands on its base, or hangs from it when the height was dragged down.
            let y = if cube { c.y + height * 0.5 * if p.height < 0.0 { -1.0 } else { 1.0 } } else { c.y };
            let _ = core.exec("move", json!({"to": [c.x, y, c.z]}));
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
        for at in dabs {
            aztek_scene::skin::stamp(&mut st.bind, &items[i].mesh, &items[i].world, at, Some(n), st.inf, &ctx, st.invert, st.smooth);
        }
        st.last = Some(wp);
        if st.applied {
            core.s.undo();
        }
        st.applied = aztek_scene::skin::commit_paint(&mut core.s.graph, st.skin, st.bind.clone()).is_ok();
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
                    "insertJoint" => Tool::InsertJoint,
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
            "multiSelect" => self.vp.multi = v.as_bool().unwrap_or(false),
            "keepSpacing" => self.vp.keep_spacing = v.as_bool().unwrap_or(true),
            "snapPivot" => self.vp.snap_pivot = v.as_bool().unwrap_or(false),
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
            "gridSize" => {
                self.vp.grid_extent = v.as_f64().unwrap_or(12.0).max(1.0);
                self.vp.reset_grid(&mut self.gpu);
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
            "manipSize" => self.vp.manip_scale = v.as_f64().unwrap_or(1.0),
            "axisOrientation" => self.vp.axis_mode = v.as_str().unwrap_or("world").to_string(),
            "curveDegree" => self.vp.curve_degree = v.as_i64().unwrap_or(3),
            "toolTab" => self.ts.tab = v.as_bool().unwrap_or(false),
            // Enter: makes the curve; the tool stays for the next.
            "finishTool" => {
                let items = gather(&mut core.s.graph);
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
            "activeHandle" => {
                // The reference's numbers: 0 X, 1 Y, 2 Z, 3 centre, then the planes.
                let h = match v.as_i64().unwrap_or(3) {
                    0 => Some(Handle::Axis(0)),
                    1 => Some(Handle::Axis(1)),
                    2 => Some(Handle::Axis(2)),
                    4 => Some(Handle::Plane(2)),
                    5 => Some(Handle::Plane(0)),
                    6 => Some(Handle::Plane(1)),
                    _ => None,
                };
                self.vp.active_handle = h;
                self.vp.hover = h;
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
                json!({"polygons": d.polygons, "surfaces": d.surfaces, "curves": d.curves, "joints": d.joints, "ikHandles": d.ik_handles, "deformers": d.deformers, "locators": d.locators, "cameras": d.cameras, "lights": d.lights, "displayLights": d.light_mode.name(), "twoSided": d.two_sided, "shadows": d.shadows, "manipulators": d.manipulators, "flat": d.flat, "textured": d.textured, "defaultMaterial": d.default_material, "boundingBox": d.bounding_box, "xray": d.xray, "xrayJoints": d.xray_joints, "xrayActive": d.xray_active, "backface": d.backface, "filmGate": d.film_gate, "resGate": d.res_gate, "safeAction": d.safe_action, "highlight": d.highlight, "grid": self.show_grid, "wireframe": self.vp.wireframe, "wireOnShaded": self.vp.wire_on_shaded})
            }
            _ => Value::Null,
        }
    }
}

/// The joint tool's click: a joint under the selected one, the parent turned to aim at it.
pub fn place_joint(core: &mut Core, at: DVec3) {
    let g = &core.s.graph;
    let parent = g.selection().last().copied().filter(|&id| aztek_scene::rig::is_joint(g, id)).map(|id| g.path(id));
    let Ok(v) = core.exec("joint", json!({"position": [at.x, at.y, at.z]})) else { return };
    if let Some(p) = parent {
        let _ = core.exec_quiet("joint", json!({"args": [p], "edit": true, "orientJoint": "xyz", "secondaryAxisOrient": "yup"}), aztek_ops::CallSource::Script);
        if let Some(j) = v.as_str() {
            let _ = core.exec_quiet("joint", json!({"args": [j], "edit": true, "orientJoint": "xyz", "secondaryAxisOrient": "yup"}), aztek_ops::CallSource::Script);
        }
    }
}

/// Component names with their mirror images, on meshes whose symmetry is on.
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

/// A draft's base: its middle, and its size along X and Z.
fn prim_box(p: &PrimDraft) -> (DVec3, DVec3) {
    let c = (p.a + p.b) * 0.5;
    (c, (p.a - p.b).abs())
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
