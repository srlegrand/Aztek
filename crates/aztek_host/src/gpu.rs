//! Vulkan: one device shared by every view, and per view a surface on a native window with its swapchain, render
//! pass and frame sync. One frame in flight per view.

use std::ffi::CStr;
use std::sync::{Arc, Mutex};

use ash::{khr, vk};
use gpu_allocator::vulkan::{Allocation, AllocationCreateDesc, AllocationScheme, Allocator, AllocatorCreateDesc};
use gpu_allocator::MemoryLocation;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

pub const DEPTH_FORMAT: vk::Format = vk::Format::D32_SFLOAT;

pub struct Buffer {
    pub buffer: vk::Buffer,
    pub alloc: Option<Allocation>,
}

/// The instance and device every view draws with.
pub struct Shared {
    pub entry: ash::Entry,
    pub instance: ash::Instance,
    pub surface_fn: khr::surface::Instance,
    pub phys: vk::PhysicalDevice,
    pub device: ash::Device,
    pub queue: vk::Queue,
    pub queue_family: u32,
    pub allocator: Arc<Mutex<Allocator>>,
    pub device_name: String,
    pub features: vk::PhysicalDeviceFeatures,
    /// Lines the device can draw as rectangles, and with smoothed edges: how wireframes, curves and manipulators are
    /// anti-aliased. Without either a line is drawn the device's own way, which multisampling may leave stepped.
    pub line_modes: (bool, bool),
    /// Textures, kept with the device so every view draws the same uploads.
    pub textures: Mutex<Option<crate::textures::TextureCache>>,
}

type Res<T> = Result<T, Box<dyn std::error::Error>>;

/// The Vulkan library. On macOS that is MoltenVK: the copy inside Aztek.app first, then a Vulkan loader on the library
/// path (the Vulkan SDK), then Homebrew's. dlopen doesn't look in Homebrew's folders by itself.
unsafe fn load_vulkan() -> Result<ash::Entry, ash::LoadingError> {
    #[cfg(target_os = "macos")]
    {
        let mut tries: Vec<std::path::PathBuf> = Vec::new();
        if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.to_path_buf())) {
            tries.push(dir.join("../Frameworks/libMoltenVK.dylib"));
            tries.push(dir.join("../Frameworks/libvulkan.1.dylib"));
        }
        for p in &tries {
            if p.exists() {
                if let Ok(e) = ash::Entry::load_from(p) {
                    return Ok(e);
                }
            }
        }
        if let Ok(e) = ash::Entry::load() {
            return Ok(e);
        }
        for p in ["/opt/homebrew/lib/libMoltenVK.dylib", "/usr/local/lib/libMoltenVK.dylib", "/opt/homebrew/lib/libvulkan.1.dylib", "/usr/local/lib/libvulkan.1.dylib"] {
            if std::path::Path::new(p).exists() {
                if let Ok(e) = ash::Entry::load_from(p) {
                    return Ok(e);
                }
            }
        }
    }
    ash::Entry::load()
}

impl Shared {
    /// display: the windowing system's display, for the instance's surface extensions. probe: a window of that display,
    /// so the chosen queue can present to it.
    pub unsafe fn new(display: RawDisplayHandle, probe: RawWindowHandle) -> Res<Arc<Shared>> {
        let entry = load_vulkan()?;
        let app_name = c"Aztek";
        let app_info =
            vk::ApplicationInfo::default().application_name(app_name).engine_name(app_name).api_version(vk::make_api_version(0, 1, 2, 0));
        let mut exts = ash_window::enumerate_required_extensions(display)?.to_vec();
        let mut flags = vk::InstanceCreateFlags::empty();
        let available: Vec<String> = entry
            .enumerate_instance_extension_properties(None)?
            .iter()
            .map(|e| CStr::from_ptr(e.extension_name.as_ptr()).to_string_lossy().into_owned())
            .collect();
        if available.iter().any(|e| e == "VK_KHR_portability_enumeration") {
            exts.push(khr::portability_enumeration::NAME.as_ptr());
            flags |= vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR;
        }
        let instance =
            entry.create_instance(&vk::InstanceCreateInfo::default().application_info(&app_info).enabled_extension_names(&exts).flags(flags), None)?;
        let surface_fn = khr::surface::Instance::new(&entry, &instance);
        let probe_surface = ash_window::create_surface(&entry, &instance, display, probe, None)?;

        // Prefer a discrete GPU, then integrated, then anything (lavapipe).
        let mut best: Option<(vk::PhysicalDevice, u32, i32)> = None;
        for pd in instance.enumerate_physical_devices()? {
            let props = instance.get_physical_device_properties(pd);
            let score = match props.device_type {
                vk::PhysicalDeviceType::DISCRETE_GPU => 3,
                vk::PhysicalDeviceType::INTEGRATED_GPU => 2,
                vk::PhysicalDeviceType::CPU => 0,
                _ => 1,
            };
            for (i, q) in instance.get_physical_device_queue_family_properties(pd).iter().enumerate() {
                let present = surface_fn.get_physical_device_surface_support(pd, i as u32, probe_surface).unwrap_or(false);
                if q.queue_flags.contains(vk::QueueFlags::GRAPHICS) && present && best.is_none_or(|b| score > b.2) {
                    best = Some((pd, i as u32, score));
                }
            }
        }
        surface_fn.destroy_surface(probe_surface, None);
        let (phys, queue_family, _) = best.ok_or("no Vulkan device can draw to this window")?;
        let device_name = CStr::from_ptr(instance.get_physical_device_properties(phys).device_name.as_ptr()).to_string_lossy().into_owned();

        let dev_exts_avail: Vec<String> = instance
            .enumerate_device_extension_properties(phys)?
            .iter()
            .map(|e| CStr::from_ptr(e.extension_name.as_ptr()).to_string_lossy().into_owned())
            .collect();
        {
            // openGLExtension answers from the device the views draw with.
            let props = instance.get_physical_device_properties(phys);
            let v = props.api_version;
            let version = format!("Vulkan {}.{}.{}", vk::api_version_major(v), vk::api_version_minor(v), vk::api_version_patch(v));
            crate::sys_info::set_gpu_info(&device_name, props.vendor_id, &version, &dev_exts_avail);
        }
        let mut dev_exts = vec![khr::swapchain::NAME.as_ptr()];
        if dev_exts_avail.iter().any(|e| e == "VK_KHR_portability_subset") {
            dev_exts.push(c"VK_KHR_portability_subset".as_ptr());
        }
        let feats = instance.get_physical_device_features(phys);
        let enabled = vk::PhysicalDeviceFeatures::default()
            .large_points(feats.large_points != 0)
            .wide_lines(feats.wide_lines != 0)
            .fill_mode_non_solid(feats.fill_mode_non_solid != 0);
        let prio = [1.0];
        let qinfo = [vk::DeviceQueueCreateInfo::default().queue_family_index(queue_family).queue_priorities(&prio)];
        // Line rasterization: asked for by name, since a device is free to step its lines under multisampling.
        let mut line_modes = (false, false);
        if dev_exts_avail.iter().any(|e| e == "VK_EXT_line_rasterization") {
            let mut lines = vk::PhysicalDeviceLineRasterizationFeaturesKHR::default();
            let mut f2 = vk::PhysicalDeviceFeatures2::default().push_next(&mut lines);
            instance.get_physical_device_features2(phys, &mut f2);
            line_modes = (lines.rectangular_lines != 0, lines.smooth_lines != 0);
        }
        let mut line_feats = vk::PhysicalDeviceLineRasterizationFeaturesKHR::default().rectangular_lines(line_modes.0).smooth_lines(line_modes.1);
        let mut info = vk::DeviceCreateInfo::default().queue_create_infos(&qinfo).enabled_features(&enabled);
        if line_modes.0 || line_modes.1 {
            dev_exts.push(c"VK_EXT_line_rasterization".as_ptr());
            info = info.push_next(&mut line_feats);
        }
        let info = info.enabled_extension_names(&dev_exts);
        let device = instance.create_device(phys, &info, None)?;
        let queue = device.get_device_queue(queue_family, 0);
        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance: instance.clone(),
            device: device.clone(),
            physical_device: phys,
            debug_settings: Default::default(),
            buffer_device_address: false,
            allocation_sizes: Default::default(),
        })?;
        Ok(Arc::new(Shared {
            entry,
            instance,
            surface_fn,
            phys,
            device,
            queue,
            queue_family,
            allocator: Arc::new(Mutex::new(allocator)),
            device_name,
            features: vk::PhysicalDeviceFeatures {
                large_points: enabled.large_points,
                wide_lines: enabled.wide_lines,
                ..Default::default()
            },
            line_modes,
            textures: Mutex::new(None),
        }))
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        if let Some(mut t) = self.textures.lock().unwrap().take() {
            t.release(&self.device, &self.allocator);
        }
    }
}

/// One view's surface and frame state. Fields named as the renderer expects.
pub struct Gpu {
    pub shared: Arc<Shared>,
    pub device: ash::Device,
    pub queue: vk::Queue,
    pub surface: vk::SurfaceKHR,
    pub swapchain_fn: khr::swapchain::Device,
    pub swapchain: vk::SwapchainKHR,
    pub format: vk::Format,
    pub extent: vk::Extent2D,
    pub images: Vec<vk::Image>,
    pub views: Vec<vk::ImageView>,
    pub depth_image: vk::Image,
    pub depth_view: vk::ImageView,
    pub depth_alloc: Option<Allocation>,
    /// Samples per pixel the view draws with; above one, it draws into `msaa_image` and resolves into the scene image.
    pub samples: vk::SampleCountFlags,
    pub msaa_image: vk::Image,
    pub msaa_view: vk::ImageView,
    pub msaa_alloc: Option<Allocation>,
    /// The pass the scene is drawn in: into `scene_image` (through `msaa_image` when multisampled), with depth kept.
    pub render_pass: vk::RenderPass,
    /// What the scene pass leaves: the view's colour at one sample a pixel, read by the post pass.
    pub scene_image: vk::Image,
    pub scene_view: vk::ImageView,
    pub scene_alloc: Option<Allocation>,
    pub scene_framebuffer: vk::Framebuffer,
    /// The pass the view's guides are drawn in after the scene (#300): the gates, the heads up display, the ViewCube,
    /// the marquee. It has the scene pass's attachments, so the scene's pipelines draw in it, but its colour starts clear
    /// and resolves into `overlay_image`, which the post pass lays over the finished picture. Depth of field, ambient
    /// occlusion and bounce light never see it; anti-aliasing still does.
    pub overlay_pass: vk::RenderPass,
    pub overlay_image: vk::Image,
    pub overlay_view: vk::ImageView,
    pub overlay_alloc: Option<Allocation>,
    /// Where the overlay pass resolves the surface colour it doesn't draw, with several samples: the scene's own
    /// surface colour is left as the scene pass resolved it.
    pub overlay_spare_image: vk::Image,
    pub overlay_spare_view: vk::ImageView,
    pub overlay_spare_alloc: Option<Allocation>,
    pub overlay_framebuffer: vk::Framebuffer,
    /// The surface's own colour, before any light, at one sample a pixel: what bounce light picks up as it leaves.
    /// Drawn through `albedo_msaa_image` when multisampled. Only the Realtime renderer writes it; elsewhere it stays 0.
    pub albedo_image: vk::Image,
    pub albedo_view: vk::ImageView,
    pub albedo_alloc: Option<Allocation>,
    pub albedo_msaa_image: vk::Image,
    pub albedo_msaa_view: vk::ImageView,
    pub albedo_msaa_alloc: Option<Allocation>,
    /// The Realtime renderer's reflection for each pixel: the fresnel-weighted reflection colour in rgb, the roughness
    /// used for its environment lookup in a (above zero wherever a material is drawn). The screen-space reflection pass
    /// reads it (#302).
    pub mat_image: vk::Image,
    pub mat_view: vk::ImageView,
    pub mat_alloc: Option<Allocation>,
    pub mat_msaa_image: vk::Image,
    pub mat_msaa_view: vk::ImageView,
    pub mat_msaa_alloc: Option<Allocation>,
    /// The rest of a material the reflection pass needs: transmission, IOR as (IOR - 1) / 2, metalness, and 1 where a
    /// Realtime material was drawn. RGBA8 is enough for these (#302).
    pub mat_aux_image: vk::Image,
    pub mat_aux_view: vk::ImageView,
    pub mat_aux_alloc: Option<Allocation>,
    pub mat_aux_msaa_image: vk::Image,
    pub mat_aux_msaa_view: vk::ImageView,
    pub mat_aux_msaa_alloc: Option<Allocation>,
    /// Where the guides' pass resolves the material attachments it doesn't draw, keeping the scene's (#302).
    pub overlay_mat_spare_image: vk::Image,
    pub overlay_mat_spare_view: vk::ImageView,
    pub overlay_mat_spare_alloc: Option<Allocation>,
    pub overlay_mat_aux_spare_image: vk::Image,
    pub overlay_mat_aux_spare_view: vk::ImageView,
    pub overlay_mat_aux_spare_alloc: Option<Allocation>,
    /// How open each pixel is to the light around it, worked out from the scene's depth before the picture is put together.
    pub ao_pass: vk::RenderPass,
    pub ao_image: vk::Image,
    pub ao_view: vk::ImageView,
    pub ao_alloc: Option<Allocation>,
    pub ao_framebuffer: vk::Framebuffer,
    /// The bounce light's second pass: what the first found, sent round once more. The picture reads this when bounce
    /// light is on.
    pub bounce_image: vk::Image,
    pub bounce_view: vk::ImageView,
    pub bounce_alloc: Option<Allocation>,
    pub bounce_framebuffer: vk::Framebuffer,
    /// Screen-space reflections and refraction, worked out from the scene's colour, depth and material before the
    /// picture is put together (#302).
    pub refl_image: vk::Image,
    pub refl_view: vk::ImageView,
    pub refl_alloc: Option<Allocation>,
    pub refl_framebuffer: vk::Framebuffer,
    /// The reflection image low-passed over a few pixels, so one bright texel does not speckle a reflection (#302).
    pub refl_soft_image: vk::Image,
    pub refl_soft_view: vk::ImageView,
    pub refl_soft_alloc: Option<Allocation>,
    pub refl_soft_framebuffer: vk::Framebuffer,
    /// A mip chain of the reflection image: each level a filtered copy, so a gather at any roughness is smooth (#302).
    pub refl_chain_image: vk::Image,
    pub refl_chain_view: vk::ImageView,
    pub refl_chain_alloc: Option<Allocation>,
    /// The scene drawn mirrored about the ground plane, for the floor's reflection (#302).
    pub planar_pass: vk::RenderPass,
    pub planar_image: vk::Image,
    pub planar_view: vk::ImageView,
    pub planar_alloc: Option<Allocation>,
    pub planar_depth_image: vk::Image,
    pub planar_depth_view: vk::ImageView,
    pub planar_depth_alloc: Option<Allocation>,
    pub planar_framebuffer: vk::Framebuffer,
    /// The scene without its transmissive surfaces, for glass to see what it lets through (#302).
    pub back_image: vk::Image,
    pub back_view: vk::ImageView,
    pub back_alloc: Option<Allocation>,
    pub back_depth_image: vk::Image,
    pub back_depth_view: vk::ImageView,
    pub back_depth_alloc: Option<Allocation>,
    pub back_framebuffer: vk::Framebuffer,
    /// The far side of the transmissive bodies: view normal in rgb, view distance in a.
    pub backface_image: vk::Image,
    pub backface_view: vk::ImageView,
    pub backface_alloc: Option<Allocation>,
    pub backface_depth_image: vk::Image,
    pub backface_depth_view: vk::ImageView,
    pub backface_depth_alloc: Option<Allocation>,
    pub backface_framebuffer: vk::Framebuffer,
    /// The pass that puts the scene on the window, through the post effects.
    pub post_pass: vk::RenderPass,
    /// One per swapchain image, for the post pass.
    pub framebuffers: Vec<vk::Framebuffer>,
    in_post: bool,
    /// Scene passes of sample counts no longer in use. Kept until the view goes: pipelines made for them are still
    /// about, and a handle freed could come back as another pass's.
    retired_passes: Vec<vk::RenderPass>,
    pub pool: vk::CommandPool,
    pub cmd: vk::CommandBuffer,
    pub fence: vk::Fence,
    pub acquired: vk::Semaphore,
    pub rendered: Vec<vk::Semaphore>,
    pub garbage: Vec<Buffer>,
}

impl Gpu {
    pub unsafe fn new(shared: Arc<Shared>, display: RawDisplayHandle, window: RawWindowHandle, width: u32, height: u32) -> Res<Gpu> {
        let device = shared.device.clone();
        let surface = ash_window::create_surface(&shared.entry, &shared.instance, display, window, None)?;
        if !shared.surface_fn.get_physical_device_surface_support(shared.phys, shared.queue_family, surface)? {
            return Err("the device can't present to this window".into());
        }
        let formats = shared.surface_fn.get_physical_device_surface_formats(shared.phys, surface)?;
        let format = formats
            .iter()
            .find(|f| f.format == vk::Format::B8G8R8A8_UNORM || f.format == vk::Format::R8G8B8A8_UNORM)
            .unwrap_or(&formats[0])
            .format;
        let samples = sample_count(&shared, wanted_samples());
        let render_pass = scene_pass(&device, samples, false)?;
        let overlay_pass = scene_pass(&device, samples, true)?;
        // The post pass: one colour attachment, the window's image, written whole.
        let post_att = [vk::AttachmentDescription::default()
            .format(format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::DONT_CARE)
            .store_op(vk::AttachmentStoreOp::STORE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)];
        let post_ref = [vk::AttachmentReference::default().attachment(0).layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];
        let post_sub = [vk::SubpassDescription::default().pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS).color_attachments(&post_ref)];
        // It reads what the scene pass wrote.
        let post_dep = [vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS)
            .dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER | vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
            .dst_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE)];
        let post_pass = device.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&post_att).subpasses(&post_sub).dependencies(&post_dep), None)?;
        // The occlusion pass: one value a pixel, read by the post pass.
        let ao_att = [vk::AttachmentDescription::default()
            .format(AO_FORMAT)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::DONT_CARE)
            .store_op(vk::AttachmentStoreOp::STORE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let ao_pass = device.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&ao_att).subpasses(&post_sub).dependencies(&post_dep), None)?;
        let planar_pass = planar_pass(&device)?;
        let pool = device.create_command_pool(
            &vk::CommandPoolCreateInfo::default().queue_family_index(shared.queue_family).flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
            None,
        )?;
        let cmd = device
            .allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(pool).level(vk::CommandBufferLevel::PRIMARY).command_buffer_count(1))?[0];
        let fence = device.create_fence(&vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED), None)?;
        let acquired = device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?;
        let swapchain_fn = khr::swapchain::Device::new(&shared.instance, &device);
        let queue = shared.queue;
        let mut gpu = Gpu {
            shared,
            device,
            queue,
            surface,
            swapchain_fn,
            swapchain: vk::SwapchainKHR::null(),
            format,
            extent: vk::Extent2D::default(),
            images: Vec::new(),
            views: Vec::new(),
            depth_image: vk::Image::null(),
            depth_view: vk::ImageView::null(),
            depth_alloc: None,
            samples,
            msaa_image: vk::Image::null(),
            msaa_view: vk::ImageView::null(),
            msaa_alloc: None,
            render_pass,
            scene_image: vk::Image::null(),
            scene_view: vk::ImageView::null(),
            scene_alloc: None,
            scene_framebuffer: vk::Framebuffer::null(),
            overlay_pass,
            overlay_image: vk::Image::null(),
            overlay_view: vk::ImageView::null(),
            overlay_alloc: None,
            overlay_spare_image: vk::Image::null(),
            overlay_spare_view: vk::ImageView::null(),
            overlay_spare_alloc: None,
            overlay_framebuffer: vk::Framebuffer::null(),
            albedo_image: vk::Image::null(),
            albedo_view: vk::ImageView::null(),
            albedo_alloc: None,
            albedo_msaa_image: vk::Image::null(),
            albedo_msaa_view: vk::ImageView::null(),
            albedo_msaa_alloc: None,
            mat_image: vk::Image::null(),
            mat_view: vk::ImageView::null(),
            mat_alloc: None,
            mat_msaa_image: vk::Image::null(),
            mat_msaa_view: vk::ImageView::null(),
            mat_msaa_alloc: None,
            mat_aux_image: vk::Image::null(),
            mat_aux_view: vk::ImageView::null(),
            mat_aux_alloc: None,
            mat_aux_msaa_image: vk::Image::null(),
            mat_aux_msaa_view: vk::ImageView::null(),
            mat_aux_msaa_alloc: None,
            overlay_mat_spare_image: vk::Image::null(),
            overlay_mat_spare_view: vk::ImageView::null(),
            overlay_mat_spare_alloc: None,
            overlay_mat_aux_spare_image: vk::Image::null(),
            overlay_mat_aux_spare_view: vk::ImageView::null(),
            overlay_mat_aux_spare_alloc: None,
            ao_pass,
            ao_image: vk::Image::null(),
            ao_view: vk::ImageView::null(),
            ao_alloc: None,
            ao_framebuffer: vk::Framebuffer::null(),
            bounce_image: vk::Image::null(),
            bounce_view: vk::ImageView::null(),
            bounce_alloc: None,
            bounce_framebuffer: vk::Framebuffer::null(),
            refl_image: vk::Image::null(),
            refl_view: vk::ImageView::null(),
            refl_alloc: None,
            refl_framebuffer: vk::Framebuffer::null(),
            refl_soft_image: vk::Image::null(),
            refl_soft_view: vk::ImageView::null(),
            refl_soft_alloc: None,
            refl_soft_framebuffer: vk::Framebuffer::null(),
            refl_chain_image: vk::Image::null(),
            refl_chain_view: vk::ImageView::null(),
            refl_chain_alloc: None,
            planar_pass,
            planar_image: vk::Image::null(),
            planar_view: vk::ImageView::null(),
            planar_alloc: None,
            planar_depth_image: vk::Image::null(),
            planar_depth_view: vk::ImageView::null(),
            planar_depth_alloc: None,
            planar_framebuffer: vk::Framebuffer::null(),
            back_image: vk::Image::null(),
            back_view: vk::ImageView::null(),
            backface_image: vk::Image::null(),
            backface_view: vk::ImageView::null(),
            backface_alloc: None,
            backface_depth_image: vk::Image::null(),
            backface_depth_view: vk::ImageView::null(),
            backface_depth_alloc: None,
            backface_framebuffer: vk::Framebuffer::null(),
            back_alloc: None,
            back_depth_image: vk::Image::null(),
            back_depth_view: vk::ImageView::null(),
            back_depth_alloc: None,
            back_framebuffer: vk::Framebuffer::null(),
            post_pass,
            framebuffers: Vec::new(),
            in_post: false,
            retired_passes: Vec::new(),
            pool,
            cmd,
            fence,
            acquired,
            rendered: Vec::new(),
            garbage: Vec::new(),
        };
        gpu.recreate_swapchain(width, height)?;
        Ok(gpu)
    }

    fn destroy_swapchain_parts(&mut self) {
        unsafe {
            for &f in &self.framebuffers {
                self.device.destroy_framebuffer(f, None);
            }
            self.framebuffers.clear();
            if self.scene_framebuffer != vk::Framebuffer::null() {
                self.device.destroy_framebuffer(self.scene_framebuffer, None);
                self.scene_framebuffer = vk::Framebuffer::null();
            }
            if self.overlay_framebuffer != vk::Framebuffer::null() {
                self.device.destroy_framebuffer(self.overlay_framebuffer, None);
                self.overlay_framebuffer = vk::Framebuffer::null();
            }
            for (view, image, alloc) in [
                (&mut self.overlay_view, self.overlay_image, &mut self.overlay_alloc),
                (&mut self.overlay_spare_view, self.overlay_spare_image, &mut self.overlay_spare_alloc),
            ] {
                if *view != vk::ImageView::null() {
                    self.device.destroy_image_view(*view, None);
                    self.device.destroy_image(image, None);
                    if let Some(a) = alloc.take() {
                        let _ = self.shared.allocator.lock().unwrap().free(a);
                    }
                    *view = vk::ImageView::null();
                }
            }
            if self.bounce_framebuffer != vk::Framebuffer::null() {
                self.device.destroy_framebuffer(self.bounce_framebuffer, None);
                self.bounce_framebuffer = vk::Framebuffer::null();
                self.device.destroy_image_view(self.bounce_view, None);
                self.device.destroy_image(self.bounce_image, None);
                if let Some(a) = self.bounce_alloc.take() {
                    let _ = self.shared.allocator.lock().unwrap().free(a);
                }
                self.bounce_view = vk::ImageView::null();
            }
            if self.refl_framebuffer != vk::Framebuffer::null() {
                self.device.destroy_framebuffer(self.refl_framebuffer, None);
                self.refl_framebuffer = vk::Framebuffer::null();
            }
            if self.refl_soft_framebuffer != vk::Framebuffer::null() {
                self.device.destroy_framebuffer(self.refl_soft_framebuffer, None);
                self.refl_soft_framebuffer = vk::Framebuffer::null();
            }
            for (view, image, alloc) in [
                (&mut self.refl_view, self.refl_image, &mut self.refl_alloc),
                (&mut self.refl_soft_view, self.refl_soft_image, &mut self.refl_soft_alloc),
                (&mut self.refl_chain_view, self.refl_chain_image, &mut self.refl_chain_alloc),
            ] {
                if *view != vk::ImageView::null() {
                    self.device.destroy_image_view(*view, None);
                    self.device.destroy_image(image, None);
                    if let Some(a) = alloc.take() {
                        let _ = self.shared.allocator.lock().unwrap().free(a);
                    }
                    *view = vk::ImageView::null();
                }
            }
            if self.planar_framebuffer != vk::Framebuffer::null() {
                self.device.destroy_framebuffer(self.planar_framebuffer, None);
                self.planar_framebuffer = vk::Framebuffer::null();
            }
            if self.back_framebuffer != vk::Framebuffer::null() {
                self.device.destroy_framebuffer(self.back_framebuffer, None);
                self.back_framebuffer = vk::Framebuffer::null();
            }
            if self.backface_framebuffer != vk::Framebuffer::null() {
                self.device.destroy_framebuffer(self.backface_framebuffer, None);
                self.backface_framebuffer = vk::Framebuffer::null();
            }
            for (view, image, alloc) in [
                (&mut self.planar_view, self.planar_image, &mut self.planar_alloc),
                (&mut self.planar_depth_view, self.planar_depth_image, &mut self.planar_depth_alloc),
                (&mut self.back_view, self.back_image, &mut self.back_alloc),
                (&mut self.back_depth_view, self.back_depth_image, &mut self.back_depth_alloc),
                (&mut self.backface_view, self.backface_image, &mut self.backface_alloc),
                (&mut self.backface_depth_view, self.backface_depth_image, &mut self.backface_depth_alloc),
            ] {
                if *view != vk::ImageView::null() {
                    self.device.destroy_image_view(*view, None);
                    self.device.destroy_image(image, None);
                    if let Some(a) = alloc.take() {
                        let _ = self.shared.allocator.lock().unwrap().free(a);
                    }
                    *view = vk::ImageView::null();
                }
            }
            if self.ao_framebuffer != vk::Framebuffer::null() {
                self.device.destroy_framebuffer(self.ao_framebuffer, None);
                self.ao_framebuffer = vk::Framebuffer::null();
            }
            if self.ao_view != vk::ImageView::null() {
                self.device.destroy_image_view(self.ao_view, None);
                self.device.destroy_image(self.ao_image, None);
                if let Some(a) = self.ao_alloc.take() {
                    let _ = self.shared.allocator.lock().unwrap().free(a);
                }
                self.ao_view = vk::ImageView::null();
            }
            for (view, image, alloc) in [
                (&mut self.albedo_view, self.albedo_image, &mut self.albedo_alloc),
                (&mut self.albedo_msaa_view, self.albedo_msaa_image, &mut self.albedo_msaa_alloc),
                (&mut self.mat_view, self.mat_image, &mut self.mat_alloc),
                (&mut self.mat_msaa_view, self.mat_msaa_image, &mut self.mat_msaa_alloc),
                (&mut self.mat_aux_view, self.mat_aux_image, &mut self.mat_aux_alloc),
                (&mut self.mat_aux_msaa_view, self.mat_aux_msaa_image, &mut self.mat_aux_msaa_alloc),
                (&mut self.overlay_mat_spare_view, self.overlay_mat_spare_image, &mut self.overlay_mat_spare_alloc),
                (&mut self.overlay_mat_aux_spare_view, self.overlay_mat_aux_spare_image, &mut self.overlay_mat_aux_spare_alloc),
            ] {
                if *view != vk::ImageView::null() {
                    self.device.destroy_image_view(*view, None);
                    self.device.destroy_image(image, None);
                    if let Some(a) = alloc.take() {
                        let _ = self.shared.allocator.lock().unwrap().free(a);
                    }
                    *view = vk::ImageView::null();
                }
            }
            if self.scene_view != vk::ImageView::null() {
                self.device.destroy_image_view(self.scene_view, None);
                self.device.destroy_image(self.scene_image, None);
                if let Some(a) = self.scene_alloc.take() {
                    let _ = self.shared.allocator.lock().unwrap().free(a);
                }
                self.scene_view = vk::ImageView::null();
            }
            for &v in &self.views {
                self.device.destroy_image_view(v, None);
            }
            self.views.clear();
            for &s in &self.rendered {
                self.device.destroy_semaphore(s, None);
            }
            self.rendered.clear();
            if self.depth_view != vk::ImageView::null() {
                self.device.destroy_image_view(self.depth_view, None);
                self.device.destroy_image(self.depth_image, None);
                if let Some(a) = self.depth_alloc.take() {
                    let _ = self.shared.allocator.lock().unwrap().free(a);
                }
                self.depth_view = vk::ImageView::null();
            }
            if self.msaa_view != vk::ImageView::null() {
                self.device.destroy_image_view(self.msaa_view, None);
                self.device.destroy_image(self.msaa_image, None);
                if let Some(a) = self.msaa_alloc.take() {
                    let _ = self.shared.allocator.lock().unwrap().free(a);
                }
                self.msaa_view = vk::ImageView::null();
            }
        }
    }

    pub fn recreate_swapchain(&mut self, width: u32, height: u32) -> Res<()> {
        unsafe {
            self.device.wait_for_fences(&[self.fence], true, u64::MAX)?;
            self.device.queue_wait_idle(self.queue)?;
            self.destroy_swapchain_parts();
            let sf = &self.shared.surface_fn;
            let caps = sf.get_physical_device_surface_capabilities(self.shared.phys, self.surface)?;
            let extent = if caps.current_extent.width != u32::MAX {
                caps.current_extent
            } else {
                vk::Extent2D {
                    width: width.clamp(caps.min_image_extent.width, caps.max_image_extent.width.max(1)),
                    height: height.clamp(caps.min_image_extent.height, caps.max_image_extent.height.max(1)),
                }
            };
            if extent.width == 0 || extent.height == 0 {
                self.extent = extent;
                return Ok(());
            }
            let mut count = caps.min_image_count + 1;
            if caps.max_image_count > 0 {
                count = count.min(caps.max_image_count);
            }
            let modes = sf.get_physical_device_surface_present_modes(self.shared.phys, self.surface)?;
            let mode = if modes.contains(&vk::PresentModeKHR::MAILBOX) { vk::PresentModeKHR::MAILBOX } else { vk::PresentModeKHR::FIFO };
            let alpha = if caps.supported_composite_alpha.contains(vk::CompositeAlphaFlagsKHR::OPAQUE) {
                vk::CompositeAlphaFlagsKHR::OPAQUE
            } else {
                vk::CompositeAlphaFlagsKHR::INHERIT
            };
            let old = self.swapchain;
            self.swapchain = self.swapchain_fn.create_swapchain(
                &vk::SwapchainCreateInfoKHR::default()
                    .surface(self.surface)
                    .min_image_count(count)
                    .image_format(self.format)
                    .image_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
                    .image_extent(extent)
                    .image_array_layers(1)
                    .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC)
                    .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .pre_transform(caps.current_transform)
                    .composite_alpha(alpha)
                    .present_mode(mode)
                    .clipped(true)
                    .old_swapchain(old),
                None,
            )?;
            if old != vk::SwapchainKHR::null() {
                self.swapchain_fn.destroy_swapchain(old, None);
            }
            self.extent = extent;
            self.images = self.swapchain_fn.get_swapchain_images(self.swapchain)?;
            for &img in &self.images {
                self.views.push(self.device.create_image_view(
                    &vk::ImageViewCreateInfo::default().image(img).view_type(vk::ImageViewType::TYPE_2D).format(self.format).subresource_range(color_range()),
                    None,
                )?);
                self.rendered.push(self.device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?);
            }
            self.depth_image = self.device.create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(DEPTH_FORMAT)
                    .extent(vk::Extent3D { width: extent.width, height: extent.height, depth: 1 })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(self.samples)
                    .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED),
                None,
            )?;
            let req = self.device.get_image_memory_requirements(self.depth_image);
            let alloc = self.shared.allocator.lock().unwrap().allocate(&AllocationCreateDesc {
                name: "depth",
                requirements: req,
                location: MemoryLocation::GpuOnly,
                linear: false,
                allocation_scheme: AllocationScheme::GpuAllocatorManaged,
            })?;
            self.device.bind_image_memory(self.depth_image, alloc.memory(), alloc.offset())?;
            self.depth_alloc = Some(alloc);
            self.depth_view = self.device.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(self.depth_image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(DEPTH_FORMAT)
                    .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::DEPTH).level_count(1).layer_count(1)),
                None,
            )?;
            let multi = self.samples != vk::SampleCountFlags::TYPE_1;
            if multi {
                self.msaa_image = self.device.create_image(
                    &vk::ImageCreateInfo::default()
                        .image_type(vk::ImageType::TYPE_2D)
                        .format(SCENE_FORMAT)
                        .extent(vk::Extent3D { width: extent.width, height: extent.height, depth: 1 })
                        .mip_levels(1)
                        .array_layers(1)
                        .samples(self.samples)
                        .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSIENT_ATTACHMENT),
                    None,
                )?;
                let req = self.device.get_image_memory_requirements(self.msaa_image);
                let alloc = self.shared.allocator.lock().unwrap().allocate(&AllocationCreateDesc {
                    name: "msaa colour",
                    requirements: req,
                    location: MemoryLocation::GpuOnly,
                    linear: false,
                    allocation_scheme: AllocationScheme::GpuAllocatorManaged,
                })?;
                self.device.bind_image_memory(self.msaa_image, alloc.memory(), alloc.offset())?;
                self.msaa_alloc = Some(alloc);
                self.msaa_view = self.device.create_image_view(
                    &vk::ImageViewCreateInfo::default().image(self.msaa_image).view_type(vk::ImageViewType::TYPE_2D).format(SCENE_FORMAT).subresource_range(color_range()),
                    None,
                )?;
            }
            // The scene's colour at one sample a pixel: what the scene pass resolves into and the post pass reads.
            self.scene_image = self.device.create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(SCENE_FORMAT)
                    .extent(vk::Extent3D { width: extent.width, height: extent.height, depth: 1 })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED),
                None,
            )?;
            let req = self.device.get_image_memory_requirements(self.scene_image);
            let alloc = self.shared.allocator.lock().unwrap().allocate(&AllocationCreateDesc {
                name: "scene colour",
                requirements: req,
                location: MemoryLocation::GpuOnly,
                linear: false,
                allocation_scheme: AllocationScheme::GpuAllocatorManaged,
            })?;
            self.device.bind_image_memory(self.scene_image, alloc.memory(), alloc.offset())?;
            self.scene_alloc = Some(alloc);
            self.scene_view = self.device.create_image_view(
                &vk::ImageViewCreateInfo::default().image(self.scene_image).view_type(vk::ImageViewType::TYPE_2D).format(SCENE_FORMAT).subresource_range(color_range()),
                None,
            )?;
            let (image, view, alloc) = self.color_image("albedo", ALBEDO_FORMAT, vk::SampleCountFlags::TYPE_1, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED, extent)?;
            (self.albedo_image, self.albedo_view, self.albedo_alloc) = (image, view, Some(alloc));
            if multi {
                let (image, view, alloc) = self.color_image("msaa albedo", ALBEDO_FORMAT, self.samples, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSIENT_ATTACHMENT, extent)?;
                (self.albedo_msaa_image, self.albedo_msaa_view, self.albedo_msaa_alloc) = (image, view, Some(alloc));
            }
            // The Realtime renderer's material: the reflection colour and roughness, and the transmission, IOR and
            // metalness the screen-space reflection pass reads (#302).
            let (image, view, alloc) = self.color_image("material", MAT_FORMAT, vk::SampleCountFlags::TYPE_1, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED, extent)?;
            (self.mat_image, self.mat_view, self.mat_alloc) = (image, view, Some(alloc));
            let (image, view, alloc) = self.color_image("material aux", MAT_AUX_FORMAT, vk::SampleCountFlags::TYPE_1, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED, extent)?;
            (self.mat_aux_image, self.mat_aux_view, self.mat_aux_alloc) = (image, view, Some(alloc));
            if multi {
                let (image, view, alloc) = self.color_image("msaa material", MAT_FORMAT, self.samples, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSIENT_ATTACHMENT, extent)?;
                (self.mat_msaa_image, self.mat_msaa_view, self.mat_msaa_alloc) = (image, view, Some(alloc));
                let (image, view, alloc) = self.color_image("msaa material aux", MAT_AUX_FORMAT, self.samples, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSIENT_ATTACHMENT, extent)?;
                (self.mat_aux_msaa_image, self.mat_aux_msaa_view, self.mat_aux_msaa_alloc) = (image, view, Some(alloc));
                let (image, view, alloc) = self.color_image("overlay material spare", MAT_FORMAT, vk::SampleCountFlags::TYPE_1, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED, extent)?;
                (self.overlay_mat_spare_image, self.overlay_mat_spare_view, self.overlay_mat_spare_alloc) = (image, view, Some(alloc));
                let (image, view, alloc) = self.color_image("overlay material aux spare", MAT_AUX_FORMAT, vk::SampleCountFlags::TYPE_1, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED, extent)?;
                (self.overlay_mat_aux_spare_image, self.overlay_mat_aux_spare_view, self.overlay_mat_aux_spare_alloc) = (image, view, Some(alloc));
            }
            let att: Vec<vk::ImageView> = if multi {
                vec![self.msaa_view, self.depth_view, self.scene_view, self.albedo_msaa_view, self.albedo_view, self.mat_msaa_view, self.mat_view, self.mat_aux_msaa_view, self.mat_aux_view]
            } else {
                vec![self.scene_view, self.depth_view, self.albedo_view, self.mat_view, self.mat_aux_view]
            };
            self.scene_framebuffer = self.device.create_framebuffer(
                &vk::FramebufferCreateInfo::default().render_pass(self.render_pass).attachments(&att).width(extent.width).height(extent.height).layers(1),
                None,
            )?;
            // The guides' own colour, laid over the picture by the post pass (#300).
            let (image, view, alloc) = self.color_image("overlay", SCENE_FORMAT, vk::SampleCountFlags::TYPE_1, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED, extent)?;
            (self.overlay_image, self.overlay_view, self.overlay_alloc) = (image, view, Some(alloc));
            let att: Vec<vk::ImageView> = if multi {
                let (image, view, alloc) = self.color_image("overlay spare", ALBEDO_FORMAT, vk::SampleCountFlags::TYPE_1, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED, extent)?;
                (self.overlay_spare_image, self.overlay_spare_view, self.overlay_spare_alloc) = (image, view, Some(alloc));
                // The scene's multisampled colour is done with once it is resolved: the guides draw into it afresh. The
                // material attachments resolve into spares, so the scene's own stay as the reflection pass reads them.
                vec![
                    self.msaa_view,
                    self.depth_view,
                    self.overlay_view,
                    self.albedo_msaa_view,
                    self.overlay_spare_view,
                    self.mat_msaa_view,
                    self.overlay_mat_spare_view,
                    self.mat_aux_msaa_view,
                    self.overlay_mat_aux_spare_view,
                ]
            } else {
                vec![self.overlay_view, self.depth_view, self.albedo_view, self.mat_view, self.mat_aux_view]
            };
            self.overlay_framebuffer = self.device.create_framebuffer(
                &vk::FramebufferCreateInfo::default().render_pass(self.overlay_pass).attachments(&att).width(extent.width).height(extent.height).layers(1),
                None,
            )?;
            self.ao_image = self.device.create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(AO_FORMAT)
                    .extent(vk::Extent3D { width: extent.width, height: extent.height, depth: 1 })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED),
                None,
            )?;
            let req = self.device.get_image_memory_requirements(self.ao_image);
            let alloc = self.shared.allocator.lock().unwrap().allocate(&AllocationCreateDesc {
                name: "occlusion",
                requirements: req,
                location: MemoryLocation::GpuOnly,
                linear: false,
                allocation_scheme: AllocationScheme::GpuAllocatorManaged,
            })?;
            self.device.bind_image_memory(self.ao_image, alloc.memory(), alloc.offset())?;
            self.ao_alloc = Some(alloc);
            self.ao_view = self.device.create_image_view(
                &vk::ImageViewCreateInfo::default().image(self.ao_image).view_type(vk::ImageViewType::TYPE_2D).format(AO_FORMAT).subresource_range(color_range()),
                None,
            )?;
            let ao_att = [self.ao_view];
            self.ao_framebuffer = self.device.create_framebuffer(
                &vk::FramebufferCreateInfo::default().render_pass(self.ao_pass).attachments(&ao_att).width(extent.width).height(extent.height).layers(1),
                None,
            )?;
            let (image, view, alloc) = self.color_image("bounce", AO_FORMAT, vk::SampleCountFlags::TYPE_1, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED, extent)?;
            (self.bounce_image, self.bounce_view, self.bounce_alloc) = (image, view, Some(alloc));
            let bounce_att = [self.bounce_view];
            self.bounce_framebuffer = self.device.create_framebuffer(
                &vk::FramebufferCreateInfo::default().render_pass(self.ao_pass).attachments(&bounce_att).width(extent.width).height(extent.height).layers(1),
                None,
            )?;
            let (image, view, alloc) = self.color_image("reflection", AO_FORMAT, vk::SampleCountFlags::TYPE_1, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED, extent)?;
            (self.refl_image, self.refl_view, self.refl_alloc) = (image, view, Some(alloc));
            let refl_att = [self.refl_view];
            self.refl_framebuffer = self.device.create_framebuffer(
                &vk::FramebufferCreateInfo::default().render_pass(self.ao_pass).attachments(&refl_att).width(extent.width).height(extent.height).layers(1),
                None,
            )?;
            let (image, view, alloc) = self.color_image("reflection soft", AO_FORMAT, vk::SampleCountFlags::TYPE_1, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED, extent)?;
            (self.refl_soft_image, self.refl_soft_view, self.refl_soft_alloc) = (image, view, Some(alloc));
            let soft_att = [self.refl_soft_view];
            self.refl_soft_framebuffer = self.device.create_framebuffer(
                &vk::FramebufferCreateInfo::default().render_pass(self.ao_pass).attachments(&soft_att).width(extent.width).height(extent.height).layers(1),
                None,
            )?;
            let (image, view, alloc) = self.mip_image("reflection mips", AO_FORMAT, 6, vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_SRC | vk::ImageUsageFlags::TRANSFER_DST, extent)?;
            (self.refl_chain_image, self.refl_chain_view, self.refl_chain_alloc) = (image, view, Some(alloc));
            // The ground plane's reflection: the mirrored scene, at one sample a pixel.
            let (image, view, alloc) = self.color_image("planar", AO_FORMAT, vk::SampleCountFlags::TYPE_1, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED, extent)?;
            (self.planar_image, self.planar_view, self.planar_alloc) = (image, view, Some(alloc));
            self.planar_depth_image = self.device.create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(DEPTH_FORMAT)
                    .extent(vk::Extent3D { width: extent.width, height: extent.height, depth: 1 })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT),
                None,
            )?;
            let req = self.device.get_image_memory_requirements(self.planar_depth_image);
            let alloc = self.shared.allocator.lock().unwrap().allocate(&AllocationCreateDesc {
                name: "planar depth",
                requirements: req,
                location: MemoryLocation::GpuOnly,
                linear: false,
                allocation_scheme: AllocationScheme::GpuAllocatorManaged,
            })?;
            self.device.bind_image_memory(self.planar_depth_image, alloc.memory(), alloc.offset())?;
            self.planar_depth_alloc = Some(alloc);
            self.planar_depth_view = self.device.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(self.planar_depth_image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(DEPTH_FORMAT)
                    .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::DEPTH).level_count(1).layer_count(1)),
                None,
            )?;
            let planar_att = [self.planar_view, self.planar_depth_view];
            self.planar_framebuffer = self.device.create_framebuffer(
                &vk::FramebufferCreateInfo::default().render_pass(self.planar_pass).attachments(&planar_att).width(extent.width).height(extent.height).layers(1),
                None,
            )?;
            // What glass lets through: the scene without the transmissive surfaces (#302).
            let (image, view, alloc) = self.color_image("behind glass", AO_FORMAT, vk::SampleCountFlags::TYPE_1, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED, extent)?;
            (self.back_image, self.back_view, self.back_alloc) = (image, view, Some(alloc));
            self.back_depth_image = self.device.create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(DEPTH_FORMAT)
                    .extent(vk::Extent3D { width: extent.width, height: extent.height, depth: 1 })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT),
                None,
            )?;
            let req = self.device.get_image_memory_requirements(self.back_depth_image);
            let alloc = self.shared.allocator.lock().unwrap().allocate(&AllocationCreateDesc {
                name: "behind depth",
                requirements: req,
                location: MemoryLocation::GpuOnly,
                linear: false,
                allocation_scheme: AllocationScheme::GpuAllocatorManaged,
            })?;
            self.device.bind_image_memory(self.back_depth_image, alloc.memory(), alloc.offset())?;
            self.back_depth_alloc = Some(alloc);
            self.back_depth_view = self.device.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(self.back_depth_image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(DEPTH_FORMAT)
                    .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::DEPTH).level_count(1).layer_count(1)),
                None,
            )?;
            let back_att = [self.back_view, self.back_depth_view];
            self.back_framebuffer = self.device.create_framebuffer(
                &vk::FramebufferCreateInfo::default().render_pass(self.planar_pass).attachments(&back_att).width(extent.width).height(extent.height).layers(1),
                None,
            )?;
            // The far side of the transmissive bodies, for glass's second interface.
            let (image, view, alloc) = self.color_image("back faces", AO_FORMAT, vk::SampleCountFlags::TYPE_1, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED, extent)?;
            (self.backface_image, self.backface_view, self.backface_alloc) = (image, view, Some(alloc));
            self.backface_depth_image = self.device.create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(DEPTH_FORMAT)
                    .extent(vk::Extent3D { width: extent.width, height: extent.height, depth: 1 })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT),
                None,
            )?;
            let req = self.device.get_image_memory_requirements(self.backface_depth_image);
            let alloc = self.shared.allocator.lock().unwrap().allocate(&AllocationCreateDesc {
                name: "back faces depth",
                requirements: req,
                location: MemoryLocation::GpuOnly,
                linear: false,
                allocation_scheme: AllocationScheme::GpuAllocatorManaged,
            })?;
            self.device.bind_image_memory(self.backface_depth_image, alloc.memory(), alloc.offset())?;
            self.backface_depth_alloc = Some(alloc);
            self.backface_depth_view = self.device.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(self.backface_depth_image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(DEPTH_FORMAT)
                    .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::DEPTH).level_count(1).layer_count(1)),
                None,
            )?;
            let att = [self.backface_view, self.backface_depth_view];
            self.backface_framebuffer = self.device.create_framebuffer(
                &vk::FramebufferCreateInfo::default().render_pass(self.planar_pass).attachments(&att).width(extent.width).height(extent.height).layers(1),
                None,
            )?;
            for &v in &self.views {
                let att = [v];
                self.framebuffers.push(self.device.create_framebuffer(
                    &vk::FramebufferCreateInfo::default().render_pass(self.post_pass).attachments(&att).width(extent.width).height(extent.height).layers(1),
                    None,
                )?);
            }
        }
        Ok(())
    }

    /// A colour image the view's size, with its memory and a view of it.
    unsafe fn color_image(&self, name: &str, format: vk::Format, samples: vk::SampleCountFlags, usage: vk::ImageUsageFlags, extent: vk::Extent2D) -> Res<(vk::Image, vk::ImageView, Allocation)> {
        let image = self.device.create_image(
            &vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(format)
                .extent(vk::Extent3D { width: extent.width, height: extent.height, depth: 1 })
                .mip_levels(1)
                .array_layers(1)
                .samples(samples)
                .usage(usage),
            None,
        )?;
        let req = self.device.get_image_memory_requirements(image);
        let alloc = self.shared.allocator.lock().unwrap().allocate(&AllocationCreateDesc {
            name,
            requirements: req,
            location: MemoryLocation::GpuOnly,
            linear: false,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })?;
        self.device.bind_image_memory(image, alloc.memory(), alloc.offset())?;
        let view = self.device.create_image_view(&vk::ImageViewCreateInfo::default().image(image).view_type(vk::ImageViewType::TYPE_2D).format(format).subresource_range(color_range()), None)?;
        Ok((image, view, alloc))
    }

    /// A colour image the view's size with a mip chain, its memory and a view of every level.
    unsafe fn mip_image(&self, name: &str, format: vk::Format, levels: u32, usage: vk::ImageUsageFlags, extent: vk::Extent2D) -> Res<(vk::Image, vk::ImageView, Allocation)> {
        let image = self.device.create_image(
            &vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(format)
                .extent(vk::Extent3D { width: extent.width, height: extent.height, depth: 1 })
                .mip_levels(levels)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .usage(usage),
            None,
        )?;
        let req = self.device.get_image_memory_requirements(image);
        let alloc = self.shared.allocator.lock().unwrap().allocate(&AllocationCreateDesc {
            name,
            requirements: req,
            location: MemoryLocation::GpuOnly,
            linear: false,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })?;
        self.device.bind_image_memory(image, alloc.memory(), alloc.offset())?;
        let range = vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(levels).layer_count(1);
        let view = self.device.create_image_view(&vk::ImageViewCreateInfo::default().image(image).view_type(vk::ImageViewType::TYPE_2D).format(format).subresource_range(range), None)?;
        Ok((image, view, alloc))
    }

    /// A host-visible buffer holding data.
    pub fn buffer_with<T: bytemuck::Pod>(&self, usage: vk::BufferUsageFlags, data: &[T]) -> Buffer {
        self.buffer_at(usage, data, MemoryLocation::CpuToGpu)
    }

    pub(crate) fn buffer_at<T: bytemuck::Pod>(&self, usage: vk::BufferUsageFlags, data: &[T], location: MemoryLocation) -> Buffer {
        let bytes: &[u8] = bytemuck::cast_slice(data);
        let size = (bytes.len() as u64).max(16);
        unsafe {
            let buffer = self
                .device
                .create_buffer(&vk::BufferCreateInfo::default().size(size).usage(usage).sharing_mode(vk::SharingMode::EXCLUSIVE), None)
                .expect("create_buffer");
            let req = self.device.get_buffer_memory_requirements(buffer);
            let alloc = self
                .shared
                .allocator
                .lock()
                .unwrap()
                .allocate(&AllocationCreateDesc {
                    name: "buffer",
                    requirements: req,
                    location,
                    linear: true,
                    allocation_scheme: AllocationScheme::GpuAllocatorManaged,
                })
                .expect("allocate");
            self.device.bind_buffer_memory(buffer, alloc.memory(), alloc.offset()).expect("bind");
            if let Some(ptr) = alloc.mapped_ptr() {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr.as_ptr() as *mut u8, bytes.len());
            }
            Buffer { buffer, alloc: Some(alloc) }
        }
    }

    pub fn drop_buffer(&mut self, b: Buffer) {
        self.garbage.push(b);
    }

    fn free_garbage(&mut self) {
        for mut b in self.garbage.drain(..) {
            unsafe { self.device.destroy_buffer(b.buffer, None) };
            if let Some(a) = b.alloc.take() {
                let _ = self.shared.allocator.lock().unwrap().free(a);
            }
        }
    }

    /// Waits for the last frame and acquires the next image, with the command buffer begun but no render pass yet. A
    /// view draws its shadow map between this and `begin_scene`. None when the swapchain needs remaking.
    pub fn begin_frame_deferred(&mut self) -> Option<u32> {
        unsafe {
            self.device.wait_for_fences(&[self.fence], true, u64::MAX).ok()?;
            self.free_garbage();
            if self.extent.width == 0 || self.extent.height == 0 {
                return None;
            }
            let (index, _) = self.swapchain_fn.acquire_next_image(self.swapchain, u64::MAX, self.acquired, vk::Fence::null()).ok()?;
            self.device.reset_fences(&[self.fence]).ok()?;
            self.device.reset_command_buffer(self.cmd, vk::CommandBufferResetFlags::empty()).ok()?;
            self.device.begin_command_buffer(self.cmd, &vk::CommandBufferBeginInfo::default()).ok()?;
            Some(index)
        }
    }

    /// Begins the scene's pass: what the view draws from here until `begin_occlusion`.
    pub fn begin_scene(&mut self, clear: [f32; 4]) {
        unsafe {
            let clears = [
                vk::ClearValue { color: vk::ClearColorValue { float32: clear } },
                // Reversed Z: far is 0.
                vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 0.0, stencil: 0 } },
                // The resolve target, not cleared.
                vk::ClearValue { color: vk::ClearColorValue { float32: clear } },
                // The albedo, none where nothing is drawn, and its resolve target.
                vk::ClearValue { color: vk::ClearColorValue { float32: [0.0; 4] } },
                vk::ClearValue { color: vk::ClearColorValue { float32: [0.0; 4] } },
                // The material, none where nothing is drawn, and its resolve target.
                vk::ClearValue { color: vk::ClearColorValue { float32: [0.0; 4] } },
                vk::ClearValue { color: vk::ClearColorValue { float32: [0.0; 4] } },
                vk::ClearValue { color: vk::ClearColorValue { float32: [0.0; 4] } },
                vk::ClearValue { color: vk::ClearColorValue { float32: [0.0; 4] } },
            ];
            let one = [clears[0], clears[1], clears[3], clears[5], clears[7]];
            let clears = if self.samples == vk::SampleCountFlags::TYPE_1 { &one[..] } else { &clears[..] };
            self.device.cmd_begin_render_pass(
                self.cmd,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.render_pass)
                    .framebuffer(self.scene_framebuffer)
                    .render_area(vk::Rect2D { offset: vk::Offset2D::default(), extent: self.extent })
                    .clear_values(clears),
                vk::SubpassContents::INLINE,
            );
            self.in_post = false;
        }
    }

    /// Waits for the last frame and acquires the next image. None when the swapchain needs remaking.
    pub fn begin_frame(&mut self, clear: [f32; 4]) -> Option<u32> {
        let index = self.begin_frame_deferred()?;
        self.begin_scene(clear);
        Some(index)
    }

    /// Ends the scene pass and begins the overlay pass: the view's guides are drawn from here until `begin_occlusion`,
    /// onto a clear layer with the scene's depth, so the post effects leave them as they are (#300).
    pub fn begin_overlay(&mut self) {
        unsafe {
            self.device.cmd_end_render_pass(self.cmd);
            let none = vk::ClearValue { color: vk::ClearColorValue { float32: [0.0; 4] } };
            let depth = vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 0.0, stencil: 0 } };
            let clears = [none, depth, none, none, none, none, none, none, none];
            let clears = if self.samples == vk::SampleCountFlags::TYPE_1 { &clears[..5] } else { &clears[..] };
            self.device.cmd_begin_render_pass(
                self.cmd,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.overlay_pass)
                    .framebuffer(self.overlay_framebuffer)
                    .render_area(vk::Rect2D { offset: vk::Offset2D::default(), extent: self.extent })
                    .clear_values(clears),
                vk::SubpassContents::INLINE,
            );
            self.device.cmd_set_viewport(self.cmd, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: self.extent.width as f32, height: self.extent.height as f32, min_depth: 0.0, max_depth: 1.0 }]);
            self.device.cmd_set_scissor(self.cmd, 0, &[vk::Rect2D { offset: vk::Offset2D::default(), extent: self.extent }]);
        }
    }

    /// Ends the scene pass (or the overlay pass after it) and begins the one that works out the occlusion from its depth.
    pub fn begin_occlusion(&mut self) {
        unsafe { self.device.cmd_end_render_pass(self.cmd) };
        self.begin_occlusion_after();
    }

    /// Begins the occlusion pass where the caller has already ended the pass before it.
    pub fn begin_occlusion_after(&mut self) {
        unsafe {
            self.device.cmd_begin_render_pass(
                self.cmd,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.ao_pass)
                    .framebuffer(self.ao_framebuffer)
                    .render_area(vk::Rect2D { offset: vk::Offset2D::default(), extent: self.extent }),
                vk::SubpassContents::INLINE,
            );
            self.device.cmd_set_viewport(self.cmd, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: self.extent.width as f32, height: self.extent.height as f32, min_depth: 0.0, max_depth: 1.0 }]);
            self.device.cmd_set_scissor(self.cmd, 0, &[vk::Rect2D { offset: vk::Offset2D::default(), extent: self.extent }]);
        }
    }

    /// Ends the reflection pass, so the image can be read and low-passed.
    pub fn end_reflection(&mut self) {
        unsafe { self.device.cmd_end_render_pass(self.cmd) };
    }

    /// Begins the low-pass of the reflection image, into `refl_soft_image`.
    pub fn begin_refl_blur(&mut self) {
        unsafe {
            self.device.cmd_begin_render_pass(
                self.cmd,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.ao_pass)
                    .framebuffer(self.refl_soft_framebuffer)
                    .render_area(vk::Rect2D { offset: vk::Offset2D::default(), extent: self.extent }),
                vk::SubpassContents::INLINE,
            );
            self.device.cmd_set_viewport(self.cmd, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: self.extent.width as f32, height: self.extent.height as f32, min_depth: 0.0, max_depth: 1.0 }]);
            self.device.cmd_set_scissor(self.cmd, 0, &[vk::Rect2D { offset: vk::Offset2D::default(), extent: self.extent }]);
        }
    }

    /// Ends the reflection low-pass.
    pub fn end_refl_blur(&mut self) {
        unsafe { self.device.cmd_end_render_pass(self.cmd) };
    }

    /// Builds the reflection mip chain from the reflection image just drawn (#302): level 1 is a bilinear half, level 2
    /// a quarter, and so on. The composite picks a level from the surface's roughness, so a gather is as smooth as the
    /// blur it stands for.
    pub fn build_reflection_mips(&mut self) {
        unsafe {
            let d = &self.device;
            let cmd = self.cmd;
            let range = |level: u32| vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: level,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            };
            let barrier = |image: vk::Image, level: u32, old: vk::ImageLayout, new: vk::ImageLayout, src: vk::PipelineStageFlags, dst: vk::PipelineStageFlags, src_access: vk::AccessFlags, dst_access: vk::AccessFlags| {
                let b = vk::ImageMemoryBarrier::default()
                    .old_layout(old)
                    .new_layout(new)
                    .src_access_mask(src_access)
                    .dst_access_mask(dst_access)
                    .image(image)
                    .subresource_range(range(level));
                d.cmd_pipeline_barrier(cmd, src, dst, vk::DependencyFlags::empty(), &[], &[], &[b]);
            };
            // What the low-pass wrote is a transfer source now.
            barrier(
                self.refl_soft_image,
                0,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::PipelineStageFlags::TRANSFER,
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                vk::AccessFlags::TRANSFER_READ,
            );
            let levels = 6u32;
            for level in 0..levels {
                let (src, src_image) = if level == 0 { (0, self.refl_soft_image) } else { (level - 1, self.refl_chain_image) };
                barrier(
                    self.refl_chain_image,
                    level,
                    vk::ImageLayout::UNDEFINED,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::AccessFlags::empty(),
                    vk::AccessFlags::TRANSFER_WRITE,
                );
                let (sw, sh) = (self.extent.width.max(1), self.extent.height.max(1));
                let src_extent = vk::Extent3D { width: (sw >> src).max(1), height: (sh >> src).max(1), depth: 1 };
                let dst_extent = vk::Extent3D { width: (sw >> level).max(1), height: (sh >> level).max(1), depth: 1 };
                let blit = vk::ImageBlit::default()
                    .src_subresource(vk::ImageSubresourceLayers::default().aspect_mask(vk::ImageAspectFlags::COLOR).mip_level(src).layer_count(1))
                    .src_offsets([vk::Offset3D { x: 0, y: 0, z: 0 }, vk::Offset3D { x: src_extent.width as i32, y: src_extent.height as i32, z: 1 }])
                    .dst_subresource(vk::ImageSubresourceLayers::default().aspect_mask(vk::ImageAspectFlags::COLOR).mip_level(level).layer_count(1))
                    .dst_offsets([vk::Offset3D { x: 0, y: 0, z: 0 }, vk::Offset3D { x: dst_extent.width as i32, y: dst_extent.height as i32, z: 1 }]);
                d.cmd_blit_image(
                    cmd,
                    src_image,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    self.refl_chain_image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[blit],
                    vk::Filter::LINEAR,
                );
                // It is the next level's source, and in the end read by the post pass.
                let next = if level + 1 < levels { vk::ImageLayout::TRANSFER_SRC_OPTIMAL } else { vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL };
                barrier(
                    self.refl_chain_image,
                    level,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    next,
                    vk::PipelineStageFlags::TRANSFER,
                    if level + 1 < levels { vk::PipelineStageFlags::TRANSFER } else { vk::PipelineStageFlags::FRAGMENT_SHADER },
                    vk::AccessFlags::TRANSFER_WRITE,
                    if level + 1 < levels { vk::AccessFlags::TRANSFER_READ } else { vk::AccessFlags::SHADER_READ },
                );
            }
            // Levels 0 to 4 were left as sources; the post pass reads them.
            for level in 0..levels - 1 {
                barrier(
                    self.refl_chain_image,
                    level,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::FRAGMENT_SHADER,
                    vk::AccessFlags::TRANSFER_READ,
                    vk::AccessFlags::SHADER_READ,
                );
            }
            barrier(
                self.refl_soft_image,
                0,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::AccessFlags::TRANSFER_READ,
                vk::AccessFlags::SHADER_READ,
            );
        }
    }

    /// Ends the occlusion pass and begins the bounce light's second, into `bounce_image`.
    pub fn begin_bounce(&mut self) {
        unsafe {
            self.device.cmd_end_render_pass(self.cmd);
            self.device.cmd_begin_render_pass(
                self.cmd,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.ao_pass)
                    .framebuffer(self.bounce_framebuffer)
                    .render_area(vk::Rect2D { offset: vk::Offset2D::default(), extent: self.extent }),
                vk::SubpassContents::INLINE,
            );
            self.device.cmd_set_viewport(self.cmd, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: self.extent.width as f32, height: self.extent.height as f32, min_depth: 0.0, max_depth: 1.0 }]);
            self.device.cmd_set_scissor(self.cmd, 0, &[vk::Rect2D { offset: vk::Offset2D::default(), extent: self.extent }]);
        }
    }

    /// Ends the bounce light's second pass and begins its low-pass along the view's rows, into `ao_image`, whose
    /// first-pass light the second pass has already read.
    pub fn begin_bounce_blur_h(&mut self) {
        unsafe {
            self.device.cmd_end_render_pass(self.cmd);
            self.device.cmd_begin_render_pass(
                self.cmd,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.ao_pass)
                    .framebuffer(self.ao_framebuffer)
                    .render_area(vk::Rect2D { offset: vk::Offset2D::default(), extent: self.extent }),
                vk::SubpassContents::INLINE,
            );
            self.device.cmd_set_viewport(self.cmd, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: self.extent.width as f32, height: self.extent.height as f32, min_depth: 0.0, max_depth: 1.0 }]);
            self.device.cmd_set_scissor(self.cmd, 0, &[vk::Rect2D { offset: vk::Offset2D::default(), extent: self.extent }]);
        }
    }

    /// Ends the row pass and begins the column pass, into `bounce_image` again. The column pass writes the image the
    /// row pass read, so a barrier waits for that read before the attachment writes.
    pub fn begin_bounce_blur_v(&mut self) {
        unsafe {
            self.device.cmd_end_render_pass(self.cmd);
            self.device.cmd_pipeline_barrier(
                self.cmd,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[],
            );
            self.device.cmd_begin_render_pass(
                self.cmd,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.ao_pass)
                    .framebuffer(self.bounce_framebuffer)
                    .render_area(vk::Rect2D { offset: vk::Offset2D::default(), extent: self.extent }),
                vk::SubpassContents::INLINE,
            );
            self.device.cmd_set_viewport(self.cmd, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: self.extent.width as f32, height: self.extent.height as f32, min_depth: 0.0, max_depth: 1.0 }]);
            self.device.cmd_set_scissor(self.cmd, 0, &[vk::Rect2D { offset: vk::Offset2D::default(), extent: self.extent }]);
        }
    }

    /// Ends the scene pass (or the guides' after it) and begins the screen-space reflections, into `refl_image`.
    pub fn begin_reflection(&mut self) {
        unsafe {
            self.device.cmd_end_render_pass(self.cmd);
            self.device.cmd_begin_render_pass(
                self.cmd,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.ao_pass)
                    .framebuffer(self.refl_framebuffer)
                    .render_area(vk::Rect2D { offset: vk::Offset2D::default(), extent: self.extent }),
                vk::SubpassContents::INLINE,
            );
            self.device.cmd_set_viewport(self.cmd, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: self.extent.width as f32, height: self.extent.height as f32, min_depth: 0.0, max_depth: 1.0 }]);
            self.device.cmd_set_scissor(self.cmd, 0, &[vk::Rect2D { offset: vk::Offset2D::default(), extent: self.extent }]);
        }
    }

    /// Begins the ground plane's reflection pass: the scene mirrored below the plane, into `planar_image` (#302).
    pub fn begin_planar(&mut self) {
        unsafe {
            let clears = [
                vk::ClearValue { color: vk::ClearColorValue { float32: [0.0; 4] } },
                vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 0.0, stencil: 0 } },
            ];
            self.device.cmd_begin_render_pass(
                self.cmd,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.planar_pass)
                    .framebuffer(self.planar_framebuffer)
                    .clear_values(&clears)
                    .render_area(vk::Rect2D { offset: vk::Offset2D::default(), extent: self.extent }),
                vk::SubpassContents::INLINE,
            );
            self.device.cmd_set_viewport(self.cmd, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: self.extent.width as f32, height: self.extent.height as f32, min_depth: 0.0, max_depth: 1.0 }]);
            self.device.cmd_set_scissor(self.cmd, 0, &[vk::Rect2D { offset: vk::Offset2D::default(), extent: self.extent }]);
        }
    }

    /// Ends the ground plane's reflection pass.
    pub fn end_planar(&mut self) {
        unsafe { self.device.cmd_end_render_pass(self.cmd) };
    }

    /// Begins the pass that draws the scene without its transmissive surfaces, for glass to see what it lets through
    /// (#302).
    pub fn begin_back(&mut self) {
        unsafe {
            self.device.cmd_begin_render_pass(
                self.cmd,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.planar_pass)
                    .framebuffer(self.back_framebuffer)
                    .render_area(vk::Rect2D { offset: vk::Offset2D::default(), extent: self.extent }),
                vk::SubpassContents::INLINE,
            );
            self.device.cmd_set_viewport(self.cmd, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: self.extent.width as f32, height: self.extent.height as f32, min_depth: 0.0, max_depth: 1.0 }]);
            self.device.cmd_set_scissor(self.cmd, 0, &[vk::Rect2D { offset: vk::Offset2D::default(), extent: self.extent }]);
        }
    }

    /// Ends the behind-glass pass.
    pub fn end_back(&mut self) {
        unsafe { self.device.cmd_end_render_pass(self.cmd) };
    }

    /// Begins the pass that records the far side of the transmissive bodies.
    pub fn begin_backface(&mut self) {
        unsafe {
            // The scene's pass is still open: it ends here, and this pass is ended by whichever pass begins next.
            self.device.cmd_end_render_pass(self.cmd);
            self.device.cmd_begin_render_pass(
                self.cmd,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.planar_pass)
                    .framebuffer(self.backface_framebuffer)
                    .render_area(vk::Rect2D { offset: vk::Offset2D::default(), extent: self.extent })
                    .clear_values(&[vk::ClearValue { color: vk::ClearColorValue { float32: [0.0; 4] } }, vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 0.0, stencil: 0 } }]),
                vk::SubpassContents::INLINE,
            );
            self.device.cmd_set_viewport(self.cmd, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: self.extent.width as f32, height: self.extent.height as f32, min_depth: 0.0, max_depth: 1.0 }]);
            self.device.cmd_set_scissor(self.cmd, 0, &[vk::Rect2D { offset: vk::Offset2D::default(), extent: self.extent }]);
        }
    }

    /// Ends the far-side pass.
    pub fn end_backface(&mut self) {
        unsafe { self.device.cmd_end_render_pass(self.cmd) };
    }

    /// Ends the occlusion pass and begins the one that puts the picture on the window: it is drawn between this and
    /// `end_frame`.
    pub fn begin_post(&mut self, index: u32) {
        unsafe {
            self.device.cmd_end_render_pass(self.cmd);
            self.device.cmd_begin_render_pass(
                self.cmd,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.post_pass)
                    .framebuffer(self.framebuffers[index as usize])
                    .render_area(vk::Rect2D { offset: vk::Offset2D::default(), extent: self.extent }),
                vk::SubpassContents::INLINE,
            );
            self.device.cmd_set_viewport(self.cmd, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: self.extent.width as f32, height: self.extent.height as f32, min_depth: 0.0, max_depth: 1.0 }]);
            self.device.cmd_set_scissor(self.cmd, 0, &[vk::Rect2D { offset: vk::Offset2D::default(), extent: self.extent }]);
        }
        self.in_post = true;
    }

    /// Changes the samples a pixel the view draws with: 1 is no anti-aliasing. The scene pass and its images are made
    /// again; whatever holds pipelines for the scene pass makes those again. False when nothing changed (the count is
    /// the one in use, or the device has no other to give).
    pub fn set_samples(&mut self, want: u32) -> Res<bool> {
        let samples = sample_count(&self.shared, want);
        if samples == self.samples {
            return Ok(false);
        }
        unsafe {
            self.device.wait_for_fences(&[self.fence], true, u64::MAX)?;
            self.device.queue_wait_idle(self.queue)?;
            let pass = scene_pass(&self.device, samples, false)?;
            self.retired_passes.push(std::mem::replace(&mut self.render_pass, pass));
            let pass = scene_pass(&self.device, samples, true)?;
            self.retired_passes.push(std::mem::replace(&mut self.overlay_pass, pass));
        }
        self.samples = samples;
        let e = self.extent;
        self.recreate_swapchain(e.width, e.height)?;
        Ok(true)
    }

    /// Samples per pixel the view draws with, as a number.
    pub fn sample_number(&self) -> u32 {
        self.samples.as_raw()
    }

    /// Ends the pass, submits and presents. False when the swapchain needs remaking.
    pub fn end_frame(&mut self, index: u32) -> bool {
        debug_assert!(self.in_post, "the post pass puts the frame on the window");
        unsafe {
            self.device.cmd_end_render_pass(self.cmd);
            if self.device.end_command_buffer(self.cmd).is_err() {
                return false;
            }
            let wait = [self.acquired];
            let stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let cmds = [self.cmd];
            let signal = [self.rendered[index as usize]];
            let submit = [vk::SubmitInfo::default().wait_semaphores(&wait).wait_dst_stage_mask(&stages).command_buffers(&cmds).signal_semaphores(&signal)];
            if self.device.queue_submit(self.queue, &submit, self.fence).is_err() {
                return false;
            }
            let swapchains = [self.swapchain];
            let indices = [index];
            let present = vk::PresentInfoKHR::default().wait_semaphores(&signal).swapchains(&swapchains).image_indices(&indices);
            matches!(self.swapchain_fn.queue_present(self.queue, &present), Ok(false))
        }
    }

    /// Reads the last presented image back as RGB rows (for screenshots and tests).
    pub fn read_back(&mut self, index: u32) -> Result<(u32, u32, Vec<u8>), String> {
        unsafe {
            let d = self.device.clone();
            d.queue_wait_idle(self.queue).map_err(|e| e.to_string())?;
            let (w, h) = (self.extent.width, self.extent.height);
            let size = (w * h * 4) as usize;
            // Memory the CPU reads quickly: an upload buffer can take seconds to read back.
            let buf = self.buffer_at(vk::BufferUsageFlags::TRANSFER_DST, &vec![0u8; size], MemoryLocation::GpuToCpu);
            let cmd = d
                .allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(self.pool).level(vk::CommandBufferLevel::PRIMARY).command_buffer_count(1))
                .map_err(|e| e.to_string())?[0];
            d.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default()).map_err(|e| e.to_string())?;
            let img = self.images[index as usize];
            let barrier = |old, new, src, dst| {
                vk::ImageMemoryBarrier::default().old_layout(old).new_layout(new).src_access_mask(src).dst_access_mask(dst).image(img).subresource_range(color_range())
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
            d.queue_submit(self.queue, &[vk::SubmitInfo::default().command_buffers(&cmds)], vk::Fence::null()).map_err(|e| e.to_string())?;
            d.queue_wait_idle(self.queue).map_err(|e| e.to_string())?;
            let ptr = buf.alloc.as_ref().unwrap().mapped_ptr().unwrap().as_ptr() as *const u8;
            let data = std::slice::from_raw_parts(ptr, size);
            let bgr = matches!(self.format, vk::Format::B8G8R8A8_UNORM | vk::Format::B8G8R8A8_SRGB);
            let mut out = Vec::with_capacity((w * h * 3) as usize);
            for px in data.chunks(4) {
                if bgr {
                    out.extend_from_slice(&[px[2], px[1], px[0]]);
                } else {
                    out.extend_from_slice(&px[..3]);
                }
            }
            d.free_command_buffers(self.pool, &cmds);
            self.drop_buffer(buf);
            Ok((w, h, out))
        }
    }
}

impl Drop for Gpu {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.queue_wait_idle(self.queue);
            self.free_garbage();
            self.destroy_swapchain_parts();
            self.swapchain_fn.destroy_swapchain(self.swapchain, None);
            self.device.destroy_semaphore(self.acquired, None);
            self.device.destroy_fence(self.fence, None);
            self.device.destroy_command_pool(self.pool, None);
            self.device.destroy_render_pass(self.render_pass, None);
            self.device.destroy_render_pass(self.overlay_pass, None);
            for p in self.retired_passes.drain(..) {
                self.device.destroy_render_pass(p, None);
            }
            self.device.destroy_render_pass(self.post_pass, None);
            self.device.destroy_render_pass(self.ao_pass, None);
            self.device.destroy_render_pass(self.planar_pass, None);
            self.shared.surface_fn.destroy_surface(self.surface, None);
        }
    }
}

/// Keep bright light for the bounce pass. Only the final picture goes to the display's format.
const SCENE_FORMAT: vk::Format = vk::Format::R16G16B16A16_SFLOAT;
/// The occlusion pass's image: bounce light in rgb, how open the pixel is in a.
pub const AO_FORMAT: vk::Format = vk::Format::R16G16B16A16_SFLOAT;
/// The surface colour the scene pass writes beside its picture.
pub const ALBEDO_FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;
/// The Realtime renderer's material: reflection colour in rgb, the roughness in a.
pub const MAT_FORMAT: vk::Format = vk::Format::R16G16B16A16_SFLOAT;
/// Transmission, IOR, metalness and whether a material is there, for the screen-space reflection pass (#302).
pub const MAT_AUX_FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;

pub fn color_range() -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1)
}

/// The pass the scene is drawn in, for a sample count: colour and depth at that count, resolved into the scene's image
/// when it is more than one. With `overlay`, the pass the view's guides are drawn in after it (#300): the same
/// attachments, so any pipeline made for the one draws in the other, but its colour starts clear, the scene's depth is
/// kept, and the single-sample surface colour is loaded and left as it was.
unsafe fn scene_pass(device: &ash::Device, samples: vk::SampleCountFlags, overlay: bool) -> Res<vk::RenderPass> {
        let multi = samples != vk::SampleCountFlags::TYPE_1;
        // With several samples: draw into a multisampled image (0) with depth (1), resolved into the scene image (2).
        let mut attachments = vec![
            vk::AttachmentDescription::default()
                .format(SCENE_FORMAT)
                .samples(samples)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(if multi { vk::AttachmentStoreOp::DONT_CARE } else { vk::AttachmentStoreOp::STORE })
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(if multi { vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL } else { vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL }),
            vk::AttachmentDescription::default()
                .format(DEPTH_FORMAT)
                .samples(samples)
                .load_op(if overlay { vk::AttachmentLoadOp::LOAD } else { vk::AttachmentLoadOp::CLEAR })
                // Kept: the post pass reads the depth for ambient occlusion.
                .store_op(vk::AttachmentStoreOp::STORE)
                .initial_layout(if overlay { vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL } else { vk::ImageLayout::UNDEFINED })
                .final_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL),
        ];
        let resolved = |format| {
            vk::AttachmentDescription::default()
                .format(format)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::DONT_CARE)
                .store_op(vk::AttachmentStoreOp::STORE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
        };
        if multi {
            attachments.push(resolved(SCENE_FORMAT));
        }
        // The surface colour beside the picture: 3 (resolved into 4) with several samples, 2 with one. The overlay pass
        // keeps the single-sample one as the scene left it; with several, it resolves into a spare image of its own.
        let keep = overlay && !multi;
        attachments.push(
            vk::AttachmentDescription::default()
                .format(ALBEDO_FORMAT)
                .samples(samples)
                .load_op(if keep { vk::AttachmentLoadOp::LOAD } else { vk::AttachmentLoadOp::CLEAR })
                .store_op(if multi { vk::AttachmentStoreOp::DONT_CARE } else { vk::AttachmentStoreOp::STORE })
                .initial_layout(if keep { vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL } else { vk::ImageLayout::UNDEFINED })
                .final_layout(if multi { vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL } else { vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL }),
        );
        if multi {
            attachments.push(resolved(ALBEDO_FORMAT));
        }
        // The material the screen-space reflection pass reads (#302). The overlay pass draws nothing into it: with one
        // sample it is loaded and stored back as it was; with several it resolves into a spare of its own.
        let keep_mat = overlay && !multi;
        for format in [MAT_FORMAT, MAT_AUX_FORMAT] {
            attachments.push(
                vk::AttachmentDescription::default()
                    .format(format)
                    .samples(samples)
                    .load_op(if keep_mat { vk::AttachmentLoadOp::LOAD } else { vk::AttachmentLoadOp::CLEAR })
                    .store_op(if multi { vk::AttachmentStoreOp::DONT_CARE } else { vk::AttachmentStoreOp::STORE })
                    .initial_layout(if keep_mat { vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL } else { vk::ImageLayout::UNDEFINED })
                    .final_layout(if multi { vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL } else { vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL }),
            );
            if multi {
                attachments.push(resolved(format));
            }
        }
        let albedo = if multi { 3 } else { 2 };
        let mat = if multi { 5 } else { 3 };
        let mat_aux = if multi { 7 } else { 4 };
        let color_ref = [
            vk::AttachmentReference::default().attachment(0).layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL),
            vk::AttachmentReference::default().attachment(albedo).layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL),
            vk::AttachmentReference::default().attachment(mat).layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL),
            vk::AttachmentReference::default().attachment(mat_aux).layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL),
        ];
        let depth_ref = vk::AttachmentReference::default().attachment(1).layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
        let resolve_ref = [
            vk::AttachmentReference::default().attachment(2).layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL),
            vk::AttachmentReference::default().attachment(4).layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL),
            vk::AttachmentReference::default().attachment(6).layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL),
            vk::AttachmentReference::default().attachment(8).layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL),
        ];
        let mut sp = vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_ref)
            .depth_stencil_attachment(&depth_ref);
        if multi {
            sp = sp.resolve_attachments(&resolve_ref);
        }
        let subpass = [sp];
        // The same for both passes, so they stay compatible: what came before (the shadow map, or the scene pass before
        // the overlay's) has finished writing colour and depth.
        let dep = [vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS)
            .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
            .dst_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            )];
        Ok(device.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&attachments).subpasses(&subpass).dependencies(&dep), None)?)
}

/// The pass the ground plane's reflection is drawn in (#302): the scene mirrored below the plane, in the display's own
/// colour at one sample a pixel, with depth of its own.
unsafe fn planar_pass(device: &ash::Device) -> Res<vk::RenderPass> {
    let attachments = [
        vk::AttachmentDescription::default()
            .format(AO_FORMAT)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
        vk::AttachmentDescription::default()
            .format(DEPTH_FORMAT)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL),
    ];
    let color_ref = [vk::AttachmentReference::default().attachment(0).layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];
    let depth_ref = vk::AttachmentReference::default().attachment(1).layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
    let subpass = [vk::SubpassDescription::default().pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS).color_attachments(&color_ref).depth_stencil_attachment(&depth_ref)];
    let dep = [vk::SubpassDependency::default()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS)
        .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS)
        .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE | vk::AccessFlags::SHADER_READ)
        .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)];
    Ok(device.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&attachments).subpasses(&subpass).dependencies(&dep), None)?)
}

/// What the environment asks for: AZTEK_MSAA=1, 2, 4 or 8. Four without it.
fn wanted_samples() -> u32 {
    std::env::var("AZTEK_MSAA").ok().and_then(|v| v.parse().ok()).unwrap_or(4)
}

/// The sample count the device gives for the one asked for: that, or the next it allows below it.
fn sample_count(shared: &Shared, want: u32) -> vk::SampleCountFlags {
    let limits = unsafe { shared.instance.get_physical_device_properties(shared.phys) }.limits;
    // displayPref -q -mhr answers with the card's own largest texture.
    aztek_scene::system_cmds::MAX_HW_TEXTURE.store(limits.max_image_dimension2_d as i64, std::sync::atomic::Ordering::Relaxed);
    let scene_samples = unsafe {
        shared.instance.get_physical_device_image_format_properties(
            shared.phys,
            SCENE_FORMAT,
            vk::ImageType::TYPE_2D,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSIENT_ATTACHMENT,
            vk::ImageCreateFlags::empty(),
        )
    }.map_or(vk::SampleCountFlags::TYPE_1, |p| p.sample_counts);
    let allowed = limits.framebuffer_color_sample_counts & limits.framebuffer_depth_sample_counts & scene_samples;
    for (n, flag) in [(8, vk::SampleCountFlags::TYPE_8), (4, vk::SampleCountFlags::TYPE_4), (2, vk::SampleCountFlags::TYPE_2)] {
        if want >= n && allowed.contains(flag) {
            return flag;
        }
    }
    vk::SampleCountFlags::TYPE_1
}
