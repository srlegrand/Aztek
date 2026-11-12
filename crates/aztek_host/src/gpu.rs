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
    /// Samples per pixel the view draws with; above one, it draws into `msaa_image` and resolves into the swapchain image.
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
    /// How open each pixel is to the light around it, worked out from the scene's depth before the picture is put together.
    pub ao_pass: vk::RenderPass,
    pub ao_image: vk::Image,
    pub ao_view: vk::ImageView,
    pub ao_alloc: Option<Allocation>,
    pub ao_framebuffer: vk::Framebuffer,
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
        let render_pass = scene_pass(&device, format, samples)?;
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
            ao_pass,
            ao_image: vk::Image::null(),
            ao_view: vk::ImageView::null(),
            ao_alloc: None,
            ao_framebuffer: vk::Framebuffer::null(),
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
                        .format(self.format)
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
                    &vk::ImageViewCreateInfo::default().image(self.msaa_image).view_type(vk::ImageViewType::TYPE_2D).format(self.format).subresource_range(color_range()),
                    None,
                )?;
            }
            // The scene's colour at one sample a pixel: what the scene pass resolves into and the post pass reads.
            self.scene_image = self.device.create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(self.format)
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
                &vk::ImageViewCreateInfo::default().image(self.scene_image).view_type(vk::ImageViewType::TYPE_2D).format(self.format).subresource_range(color_range()),
                None,
            )?;
            let att: Vec<vk::ImageView> = if multi { vec![self.msaa_view, self.depth_view, self.scene_view] } else { vec![self.scene_view, self.depth_view] };
            self.scene_framebuffer = self.device.create_framebuffer(
                &vk::FramebufferCreateInfo::default().render_pass(self.render_pass).attachments(&att).width(extent.width).height(extent.height).layers(1),
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

    /// Waits for the last frame and acquires the next image. None when the swapchain needs remaking.
    pub fn begin_frame(&mut self, clear: [f32; 4]) -> Option<u32> {
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
            let clears = [
                vk::ClearValue { color: vk::ClearColorValue { float32: clear } },
                // Reversed Z: far is 0.
                vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 0.0, stencil: 0 } },
                // The resolve target, not cleared.
                vk::ClearValue { color: vk::ClearColorValue { float32: clear } },
            ];
            let clears = if self.samples == vk::SampleCountFlags::TYPE_1 { &clears[..2] } else { &clears[..] };
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
            Some(index)
        }
    }

    /// Ends the scene pass and begins the one that works out the occlusion from its depth.
    pub fn begin_occlusion(&mut self) {
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
            let pass = scene_pass(&self.device, self.format, samples)?;
            self.retired_passes.push(std::mem::replace(&mut self.render_pass, pass));
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
            for p in self.retired_passes.drain(..) {
                self.device.destroy_render_pass(p, None);
            }
            self.device.destroy_render_pass(self.post_pass, None);
            self.device.destroy_render_pass(self.ao_pass, None);
            self.shared.surface_fn.destroy_surface(self.surface, None);
        }
    }
}

/// One number a pixel: how open it is.
pub const AO_FORMAT: vk::Format = vk::Format::R8_UNORM;

pub fn color_range() -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1)
}

/// The pass the scene is drawn in, for a sample count: colour and depth at that count, resolved into the scene's image
/// when it is more than one.
unsafe fn scene_pass(device: &ash::Device, format: vk::Format, samples: vk::SampleCountFlags) -> Res<vk::RenderPass> {
        let multi = samples != vk::SampleCountFlags::TYPE_1;
        // With several samples: draw into a multisampled image (0) with depth (1), resolved into the swapchain image (2).
        let mut attachments = vec![
            vk::AttachmentDescription::default()
                .format(format)
                .samples(samples)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(if multi { vk::AttachmentStoreOp::DONT_CARE } else { vk::AttachmentStoreOp::STORE })
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(if multi { vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL } else { vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL }),
            vk::AttachmentDescription::default()
                .format(DEPTH_FORMAT)
                .samples(samples)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                // Kept: the post pass reads the depth for ambient occlusion.
                .store_op(vk::AttachmentStoreOp::STORE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL),
        ];
        if multi {
            attachments.push(
                vk::AttachmentDescription::default()
                    .format(format)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .load_op(vk::AttachmentLoadOp::DONT_CARE)
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .initial_layout(vk::ImageLayout::UNDEFINED)
                    .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
            );
        }
        let color_ref = [vk::AttachmentReference::default().attachment(0).layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];
        let depth_ref = vk::AttachmentReference::default().attachment(1).layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
        let resolve_ref = [vk::AttachmentReference::default().attachment(2).layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];
        let mut sp = vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_ref)
            .depth_stencil_attachment(&depth_ref);
        if multi {
            sp = sp.resolve_attachments(&resolve_ref);
        }
        let subpass = [sp];
        let dep = [vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS)
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
    let allowed = limits.framebuffer_color_sample_counts & limits.framebuffer_depth_sample_counts;
    for (n, flag) in [(8, vk::SampleCountFlags::TYPE_8), (4, vk::SampleCountFlags::TYPE_4), (2, vk::SampleCountFlags::TYPE_2)] {
        if want >= n && allowed.contains(flag) {
            return flag;
        }
    }
    vk::SampleCountFlags::TYPE_1
}
