//! Vulkan device, swapchain, render pass and per-frame sync. One frame in flight.

use std::ffi::CStr;
use std::sync::{Arc, Mutex};

use ash::{khr, vk};
use gpu_allocator::vulkan::{Allocation, AllocationCreateDesc, AllocationScheme, Allocator, AllocatorCreateDesc};
use gpu_allocator::MemoryLocation;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::Window;

pub const DEPTH_FORMAT: vk::Format = vk::Format::D32_SFLOAT;

pub struct Buffer {
    pub buffer: vk::Buffer,
    pub alloc: Option<Allocation>,
    #[allow(dead_code)]
    pub size: u64,
}

pub struct Gpu {
    pub _entry: ash::Entry,
    pub instance: ash::Instance,
    pub surface_fn: khr::surface::Instance,
    pub surface: vk::SurfaceKHR,
    pub phys: vk::PhysicalDevice,
    pub device: ash::Device,
    pub queue: vk::Queue,
    #[allow(dead_code)]
    pub queue_family: u32,
    pub swapchain_fn: khr::swapchain::Device,
    pub swapchain: vk::SwapchainKHR,
    pub format: vk::Format,
    pub extent: vk::Extent2D,
    pub images: Vec<vk::Image>,
    pub views: Vec<vk::ImageView>,
    pub depth_image: vk::Image,
    pub depth_view: vk::ImageView,
    pub depth_alloc: Option<Allocation>,
    pub render_pass: vk::RenderPass,
    pub framebuffers: Vec<vk::Framebuffer>,
    pub pool: vk::CommandPool,
    pub cmd: vk::CommandBuffer,
    pub fence: vk::Fence,
    pub acquired: vk::Semaphore,
    /// One per swapchain image: presentation may still hold the previous one.
    pub rendered: Vec<vk::Semaphore>,
    pub allocator: Arc<Mutex<Allocator>>,
    /// Buffers dropped this frame, freed once the frame's fence has passed.
    pub garbage: Vec<Buffer>,
    pub device_name: String,
}

impl Gpu {
    pub fn new(window: &Window) -> Result<Gpu, String> {
        unsafe { Self::create(window) }.map_err(|e| e.to_string())
    }

    unsafe fn create(window: &Window) -> Result<Gpu, Box<dyn std::error::Error>> {
        let entry = ash::Entry::load()?;
        let display = window.display_handle()?.as_raw();
        let app_name = c"Aztek";
        let app_info = vk::ApplicationInfo::default()
            .application_name(app_name)
            .engine_name(app_name)
            .api_version(vk::make_api_version(0, 1, 2, 0));
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
        let instance = entry.create_instance(
            &vk::InstanceCreateInfo::default().application_info(&app_info).enabled_extension_names(&exts).flags(flags),
            None,
        )?;
        let surface = ash_window::create_surface(&entry, &instance, display, window.window_handle()?.as_raw(), None)?;
        let surface_fn = khr::surface::Instance::new(&entry, &instance);

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
                let present = surface_fn.get_physical_device_surface_support(pd, i as u32, surface).unwrap_or(false);
                if q.queue_flags.contains(vk::QueueFlags::GRAPHICS) && present && best.is_none_or(|b| score > b.2) {
                    best = Some((pd, i as u32, score));
                }
            }
        }
        let (phys, queue_family, _) = best.ok_or("no Vulkan device can draw to this window")?;
        let device_name = CStr::from_ptr(instance.get_physical_device_properties(phys).device_name.as_ptr())
            .to_string_lossy()
            .into_owned();

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
        let device = instance.create_device(
            phys,
            &vk::DeviceCreateInfo::default().queue_create_infos(&qinfo).enabled_extension_names(&dev_exts).enabled_features(&enabled),
            None,
        )?;
        let queue = device.get_device_queue(queue_family, 0);
        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance: instance.clone(),
            device: device.clone(),
            physical_device: phys,
            debug_settings: Default::default(),
            buffer_device_address: false,
            allocation_sizes: Default::default(),
        })?;
        let swapchain_fn = khr::swapchain::Device::new(&instance, &device);

        let formats = surface_fn.get_physical_device_surface_formats(phys, surface)?;
        let format = formats
            .iter()
            .find(|f| f.format == vk::Format::B8G8R8A8_UNORM || f.format == vk::Format::R8G8B8A8_UNORM)
            .unwrap_or(&formats[0])
            .format;

        let attachments = [
            vk::AttachmentDescription::default()
                .format(format)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::STORE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::PRESENT_SRC_KHR),
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
        let subpass = [vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_ref)
            .depth_stencil_attachment(&depth_ref)];
        let dep = [vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS)
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)];
        let render_pass = device.create_render_pass(
            &vk::RenderPassCreateInfo::default().attachments(&attachments).subpasses(&subpass).dependencies(&dep),
            None,
        )?;

        let pool = device.create_command_pool(
            &vk::CommandPoolCreateInfo::default()
                .queue_family_index(queue_family)
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
            None,
        )?;
        let cmd = device.allocate_command_buffers(
            &vk::CommandBufferAllocateInfo::default().command_pool(pool).level(vk::CommandBufferLevel::PRIMARY).command_buffer_count(1),
        )?[0];
        let fence = device.create_fence(&vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED), None)?;
        let acquired = device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?;

        let mut gpu = Gpu {
            _entry: entry,
            instance,
            surface_fn,
            surface,
            phys,
            device,
            queue,
            queue_family,
            swapchain_fn,
            swapchain: vk::SwapchainKHR::null(),
            format,
            extent: vk::Extent2D::default(),
            images: Vec::new(),
            views: Vec::new(),
            depth_image: vk::Image::null(),
            depth_view: vk::ImageView::null(),
            depth_alloc: None,
            render_pass,
            framebuffers: Vec::new(),
            pool,
            cmd,
            fence,
            acquired,
            rendered: Vec::new(),
            allocator: Arc::new(Mutex::new(allocator)),
            garbage: Vec::new(),
            device_name,
        };
        let size = window.inner_size();
        gpu.recreate_swapchain(size.width, size.height)?;
        Ok(gpu)
    }

    fn destroy_swapchain_parts(&mut self) {
        unsafe {
            for &f in &self.framebuffers {
                self.device.destroy_framebuffer(f, None);
            }
            self.framebuffers.clear();
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
                    let _ = self.allocator.lock().unwrap().free(a);
                }
                self.depth_view = vk::ImageView::null();
            }
        }
    }

    pub fn recreate_swapchain(&mut self, width: u32, height: u32) -> Result<(), Box<dyn std::error::Error>> {
        unsafe {
            self.device.device_wait_idle()?;
            self.destroy_swapchain_parts();
            let caps = self.surface_fn.get_physical_device_surface_capabilities(self.phys, self.surface)?;
            let extent = if caps.current_extent.width != u32::MAX {
                caps.current_extent
            } else {
                vk::Extent2D {
                    width: width.clamp(caps.min_image_extent.width, caps.max_image_extent.width),
                    height: height.clamp(caps.min_image_extent.height, caps.max_image_extent.height),
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
            let modes = self.surface_fn.get_physical_device_surface_present_modes(self.phys, self.surface)?;
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
                    &vk::ImageViewCreateInfo::default()
                        .image(img)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(self.format)
                        .subresource_range(color_range()),
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
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT),
                None,
            )?;
            let req = self.device.get_image_memory_requirements(self.depth_image);
            let alloc = self.allocator.lock().unwrap().allocate(&AllocationCreateDesc {
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
                    .subresource_range(
                        vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::DEPTH).level_count(1).layer_count(1),
                    ),
                None,
            )?;
            for &v in &self.views {
                let att = [v, self.depth_view];
                self.framebuffers.push(self.device.create_framebuffer(
                    &vk::FramebufferCreateInfo::default()
                        .render_pass(self.render_pass)
                        .attachments(&att)
                        .width(extent.width)
                        .height(extent.height)
                        .layers(1),
                    None,
                )?);
            }
        }
        Ok(())
    }

    /// A host-visible buffer holding data.
    pub fn buffer_with<T: bytemuck::Pod>(&self, usage: vk::BufferUsageFlags, data: &[T]) -> Buffer {
        let bytes: &[u8] = bytemuck::cast_slice(data);
        let size = (bytes.len() as u64).max(16);
        unsafe {
            let buffer = self
                .device
                .create_buffer(&vk::BufferCreateInfo::default().size(size).usage(usage).sharing_mode(vk::SharingMode::EXCLUSIVE), None)
                .expect("create_buffer");
            let req = self.device.get_buffer_memory_requirements(buffer);
            let alloc = self
                .allocator
                .lock()
                .unwrap()
                .allocate(&AllocationCreateDesc {
                    name: "buffer",
                    requirements: req,
                    location: MemoryLocation::CpuToGpu,
                    linear: true,
                    allocation_scheme: AllocationScheme::GpuAllocatorManaged,
                })
                .expect("allocate");
            self.device.bind_buffer_memory(buffer, alloc.memory(), alloc.offset()).expect("bind");
            if let Some(ptr) = alloc.mapped_ptr() {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr.as_ptr() as *mut u8, bytes.len());
            }
            Buffer { buffer, alloc: Some(alloc), size }
        }
    }

    pub fn drop_buffer(&mut self, b: Buffer) {
        self.garbage.push(b);
    }

    fn free_garbage(&mut self) {
        for mut b in self.garbage.drain(..) {
            unsafe { self.device.destroy_buffer(b.buffer, None) };
            if let Some(a) = b.alloc.take() {
                let _ = self.allocator.lock().unwrap().free(a);
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
            let (index, _) = match self.swapchain_fn.acquire_next_image(self.swapchain, u64::MAX, self.acquired, vk::Fence::null()) {
                Ok(r) => r,
                Err(_) => return None,
            };
            self.device.reset_fences(&[self.fence]).ok()?;
            self.device.reset_command_buffer(self.cmd, vk::CommandBufferResetFlags::empty()).ok()?;
            self.device.begin_command_buffer(self.cmd, &vk::CommandBufferBeginInfo::default()).ok()?;
            let clears = [
                vk::ClearValue { color: vk::ClearColorValue { float32: clear } },
                // Reversed Z: far is 0.
                vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 0.0, stencil: 0 } },
            ];
            self.device.cmd_begin_render_pass(
                self.cmd,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.render_pass)
                    .framebuffer(self.framebuffers[index as usize])
                    .render_area(vk::Rect2D { offset: vk::Offset2D::default(), extent: self.extent })
                    .clear_values(&clears),
                vk::SubpassContents::INLINE,
            );
            Some(index)
        }
    }

    /// Ends the pass, submits and presents. False when the swapchain needs remaking.
    pub fn end_frame(&mut self, index: u32) -> bool {
        unsafe {
            self.device.cmd_end_render_pass(self.cmd);
            if self.device.end_command_buffer(self.cmd).is_err() {
                return false;
            }
            let wait = [self.acquired];
            let stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let cmds = [self.cmd];
            let signal = [self.rendered[index as usize]];
            let submit = [vk::SubmitInfo::default()
                .wait_semaphores(&wait)
                .wait_dst_stage_mask(&stages)
                .command_buffers(&cmds)
                .signal_semaphores(&signal)];
            if self.device.queue_submit(self.queue, &submit, self.fence).is_err() {
                return false;
            }
            let swapchains = [self.swapchain];
            let indices = [index];
            let present = vk::PresentInfoKHR::default().wait_semaphores(&signal).swapchains(&swapchains).image_indices(&indices);
            matches!(self.swapchain_fn.queue_present(self.queue, &present), Ok(false))
        }
    }
}

impl Drop for Gpu {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            self.free_garbage();
            self.destroy_swapchain_parts();
            self.swapchain_fn.destroy_swapchain(self.swapchain, None);
            self.device.destroy_semaphore(self.acquired, None);
            self.device.destroy_fence(self.fence, None);
            self.device.destroy_command_pool(self.pool, None);
            self.device.destroy_render_pass(self.render_pass, None);
        }
        // The allocator and device go when the app tears down; the OS reclaims the rest.
    }
}

pub fn color_range() -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1)
}
