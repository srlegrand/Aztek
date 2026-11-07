//! Aztek: the application window.

mod app;
mod camera;
mod gpu;
mod mel;
mod render;
mod viewport;

use std::sync::Arc;

use aztek_scene::Session;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::app::Ui;
use crate::gpu::Gpu;
use crate::render::Renderer;
use crate::viewport::{gather, Viewport};

struct State {
    // Drop order: GPU users before the device.
    egui_renderer: egui_ash_renderer::Renderer<egui_ash_renderer::allocator::GpuAllocator>,
    renderer: Renderer,
    viewport: Viewport,
    gpu: Gpu,
    egui_ctx: egui::Context,
    egui_winit: egui_winit::State,
    ui: Ui,
    session: Session,
    window: Arc<Window>,
    resized: bool,
    /// Frames still to grab for --screenshot, then the path.
    screenshot: Option<(u32, String)>,
    script: Option<String>,
}

struct App {
    state: Option<State>,
    args: Args,
}

#[derive(Default)]
struct Args {
    screenshot: Option<String>,
    script: Option<String>,
    file: Option<String>,
    log: bool,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let attrs = Window::default_attributes().with_title("Aztek").with_inner_size(winit::dpi::LogicalSize::new(1600.0, 960.0));
        let window = Arc::new(el.create_window(attrs).expect("window"));
        let gpu = match Gpu::new(&window) {
            Ok(g) => g,
            Err(e) => {
                eprintln!("aztek: Vulkan start-up failed: {e}");
                el.exit();
                return;
            }
        };
        log::info!("Vulkan device: {}", gpu.device_name);
        let renderer = Renderer::new(&gpu);
        let egui_renderer = egui_ash_renderer::Renderer::with_gpu_allocator(
            gpu.allocator.clone(),
            gpu.device.clone(),
            egui_ash_renderer::RenderMode::RenderPass(gpu.render_pass),
            egui_ash_renderer::Options { in_flight_frames: 1, srgb_framebuffer: false, ..Default::default() },
        )
        .expect("egui renderer");
        let egui_ctx = egui::Context::default();
        egui_ctx.set_visuals(egui::Visuals::dark());
        let egui_winit = egui_winit::State::new(egui_ctx.clone(), egui::ViewportId::ROOT, &window, Some(window.scale_factor() as f32), None, None);
        let mut ui = Ui::new();
        let session = Session::new();
        ui.history.push(app::Line { text: format!("// Vulkan device: {} //", gpu.device_name), kind: app::LineKind::Info });
        self.state = Some(State {
            egui_renderer,
            renderer,
            viewport: Viewport::new(),
            gpu,
            egui_ctx,
            egui_winit,
            ui,
            session,
            window,
            resized: false,
            screenshot: self.args.screenshot.clone().map(|p| (4, p)),
            script: self.args.script.clone(),
        });
        if let Some(st) = &mut self.state {
            if let Some(f) = self.args.file.clone() {
                st.ui.current_file = None;
                st.ui.history.push(app::Line { text: format!("// Opening {f} //"), kind: app::LineKind::Info });
                open_file(st, &f);
            }
            st.window.request_redraw();
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(st) = &mut self.state else { return };
        let resp = st.egui_winit.on_window_event(&st.window, &event);
        match event {
            WindowEvent::CloseRequested => {
                el.exit();
                return;
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => st.resized = true,
            WindowEvent::RedrawRequested => {
                frame(st);
                if st.ui.quit {
                    el.exit();
                }
                if let Some((0, _)) = st.screenshot {
                    el.exit();
                }
                return;
            }
            _ => {}
        }
        if resp.repaint {
            st.window.request_redraw();
        }
    }

    fn about_to_wait(&mut self, _el: &ActiveEventLoop) {
        if let Some(st) = &self.state {
            if st.screenshot.is_some() {
                st.window.request_redraw();
            }
        }
    }

    fn exiting(&mut self, _el: &ActiveEventLoop) {
        if let Some(mut st) = self.state.take() {
            if self.args.log {
                for l in &st.ui.history {
                    println!("{}", l.text);
                }
            }
            unsafe {
                let _ = st.gpu.device.device_wait_idle();
            }
            st.viewport.release(&mut st.gpu);
            drop(st);
        }
    }
}

fn open_file(st: &mut State, path: &str) {
    st.ui.open_path(&mut st.session, path);
}

fn frame(st: &mut State) {
    if st.resized {
        let size = st.window.inner_size();
        if let Err(e) = st.gpu.recreate_swapchain(size.width, size.height) {
            log::error!("swapchain: {e}");
        }
        st.resized = false;
    }
    if let Some(src) = st.script.take() {
        let text = std::fs::read_to_string(&src).unwrap_or_else(|e| format!("// {src}: {e}"));
        st.ui.run_source(&mut st.session, &text);
        let items = gather(&mut st.session.graph);
        st.viewport.frame(&mut st.session, &items, true);
    }
    let raw = st.egui_winit.take_egui_input(&st.window);
    let mut output = st.egui_ctx.run_ui(raw, |ui| {
        st.ui.build(ui, &mut st.session, &mut st.viewport);
    });
    st.egui_winit.handle_platform_output(&st.window, output.platform_output.clone());
    let ppp = output.pixels_per_point;
    let prims = st.egui_ctx.tessellate(output.shapes, ppp);

    let mut tex = std::mem::take(&mut output.textures_delta);
    unsafe {
        let _ = st.gpu.device.wait_for_fences(&[st.gpu.fence], true, u64::MAX);
    }
    for (id, deltas) in &tex.set {
        for delta in deltas {
            if let Err(e) = st.egui_renderer.set_texture(st.gpu.queue, st.gpu.pool, *id, delta) {
                log::error!("egui texture: {e}");
            }
        }
    }
    let bg = [0.36f32, 0.36, 0.36, 1.0];
    let Some(index) = st.gpu.begin_frame(bg) else {
        for id in &tex.free {
            st.egui_renderer.free_texture(*id).ok();
        }
        tex.clear();
        st.resized = true;
        st.window.request_redraw();
        return;
    };
    let items = gather(&mut st.session.graph);
    st.viewport.draw(&mut st.gpu, &st.renderer, &mut st.session, &items, ppp as f64);
    let extent = st.gpu.extent;
    if let Err(e) = st.egui_renderer.cmd_draw(st.gpu.cmd, extent, ppp, &prims) {
        log::error!("egui draw: {e}");
    }
    let shot = matches!(st.screenshot, Some((1, _)));
    if shot {
        // Copy happens after the pass; end_frame ends the pass, so grab via a second submission below.
    }
    if !st.gpu.end_frame(index) {
        st.resized = true;
    }
    if shot {
        let path = st.screenshot.as_ref().unwrap().1.clone();
        if let Err(e) = save_screenshot(&st.gpu, index, &path) {
            eprintln!("screenshot: {e}");
        }
    }
    if let Some((n, _)) = &mut st.screenshot {
        *n = n.saturating_sub(1);
    }
    unsafe {
        let _ = st.gpu.device.wait_for_fences(&[st.gpu.fence], true, u64::MAX);
    }
    for id in &tex.free {
        st.egui_renderer.free_texture(*id).ok();
    }
    tex.clear();
    if output.viewport_output.values().any(|v| v.repaint_delay.is_zero()) || st.ui_busy() {
        st.window.request_redraw();
    }
}

impl State {
    /// Whether something needs continuous redraws (a drag in progress).
    fn ui_busy(&self) -> bool {
        self.viewport.drag.is_some()
    }
}

/// Reads the presented image back into a PPM file (for headless checks).
fn save_screenshot(gpu: &Gpu, index: u32, path: &str) -> Result<(), String> {
    use ash::vk;
    unsafe {
        let d = &gpu.device;
        d.queue_wait_idle(gpu.queue).map_err(|e| e.to_string())?;
        let (w, h) = (gpu.extent.width, gpu.extent.height);
        let size = (w * h * 4) as u64;
        let buf = gpu.buffer_with(vk::BufferUsageFlags::TRANSFER_DST, &vec![0u8; size as usize]);
        let cmd = d
            .allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(gpu.pool).level(vk::CommandBufferLevel::PRIMARY).command_buffer_count(1))
            .map_err(|e| e.to_string())?[0];
        d.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default()).map_err(|e| e.to_string())?;
        let img = gpu.images[index as usize];
        let barrier = |old, new, src, dst| {
            vk::ImageMemoryBarrier::default()
                .old_layout(old)
                .new_layout(new)
                .src_access_mask(src)
                .dst_access_mask(dst)
                .image(img)
                .subresource_range(gpu::color_range())
        };
        d.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::ALL_COMMANDS,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier(vk::ImageLayout::PRESENT_SRC_KHR, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, vk::AccessFlags::empty(), vk::AccessFlags::TRANSFER_READ)],
        );
        d.cmd_copy_image_to_buffer(
            cmd,
            img,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            buf.buffer,
            &[vk::BufferImageCopy::default()
                .image_subresource(vk::ImageSubresourceLayers::default().aspect_mask(vk::ImageAspectFlags::COLOR).layer_count(1))
                .image_extent(vk::Extent3D { width: w, height: h, depth: 1 })],
        );
        d.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::ALL_COMMANDS,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier(vk::ImageLayout::TRANSFER_SRC_OPTIMAL, vk::ImageLayout::PRESENT_SRC_KHR, vk::AccessFlags::TRANSFER_READ, vk::AccessFlags::empty())],
        );
        d.end_command_buffer(cmd).map_err(|e| e.to_string())?;
        let cmds = [cmd];
        d.queue_submit(gpu.queue, &[vk::SubmitInfo::default().command_buffers(&cmds)], vk::Fence::null()).map_err(|e| e.to_string())?;
        d.queue_wait_idle(gpu.queue).map_err(|e| e.to_string())?;
        let ptr = buf.alloc.as_ref().unwrap().mapped_ptr().unwrap().as_ptr() as *const u8;
        let data = std::slice::from_raw_parts(ptr, size as usize);
        let bgr = matches!(gpu.format, vk::Format::B8G8R8A8_UNORM | vk::Format::B8G8R8A8_SRGB);
        let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
        for px in data.chunks(4) {
            if bgr {
                out.extend_from_slice(&[px[2], px[1], px[0]]);
            } else {
                out.extend_from_slice(&px[..3]);
            }
        }
        std::fs::write(path, out).map_err(|e| e.to_string())?;
        d.free_command_buffers(gpu.pool, &cmds);
        d.destroy_buffer(buf.buffer, None);
        if let Some(a) = buf.alloc {
            let _ = gpu.allocator.lock().unwrap().free(a);
        }
    }
    Ok(())
}

fn parse_args() -> Args {
    let mut a = Args::default();
    let mut it = std::env::args().skip(1);
    while let Some(x) = it.next() {
        match x.as_str() {
            "--screenshot" => a.screenshot = it.next(),
            "--script" => a.script = it.next(),
            "--log" => a.log = true,
            "-h" | "--help" => {
                println!("aztek [scene.aztek] [--script commands.mel] [--screenshot out.ppm]");
                std::process::exit(0);
            }
            f => a.file = Some(f.to_string()),
        }
    }
    a
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let args = parse_args();
    let el = EventLoop::new().expect("event loop");
    el.set_control_flow(ControlFlow::Wait);
    let mut app = App { state: None, args };
    el.run_app(&mut app).expect("run");
}
