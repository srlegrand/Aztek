//! Image files for the viewport: decoded in Rust, uploaded with their mip chain, kept per path and modification time, so
//! a file saved over is picked up. One cache per device.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

use ash::vk;
use gpu_allocator::vulkan::{Allocation, AllocationCreateDesc, AllocationScheme};
use gpu_allocator::MemoryLocation;

use crate::gpu::Gpu;

pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// The image has an alpha channel with something in it.
    pub has_alpha: bool,
}

/// An image file as 8-bit RGBA, top row first.
pub fn decode(path: &str) -> Result<Decoded, String> {
    let reader = image::ImageReader::open(path).map_err(|e| e.to_string())?.with_guessed_format().map_err(|e| e.to_string())?;
    let img = reader.decode().map_err(|e| e.to_string())?;
    let declares = img.color().has_alpha();
    let rgba = img.into_rgba8();
    let (width, height) = rgba.dimensions();
    let has_alpha = declares && rgba.pixels().any(|p| p.0[3] != 255);
    Ok(Decoded { width, height, rgba: rgba.into_raw(), has_alpha })
}

struct Entry {
    image: vk::Image,
    view: vk::ImageView,
    alloc: Option<Allocation>,
    set: vk::DescriptorSet,
    mtime: Option<SystemTime>,
    checked: Instant,
    has_alpha: bool,
    size: (u32, u32),
}

/// What a material needs of a texture.
#[derive(Clone, Copy)]
pub struct Loaded {
    pub set: vk::DescriptorSet,
    pub has_alpha: bool,
    pub size: (u32, u32),
}

pub struct TextureCache {
    layout: vk::DescriptorSetLayout,
    pool: vk::DescriptorPool,
    sampler: vk::Sampler,
    commands: vk::CommandPool,
    /// Bound where a material has no texture: one white texel.
    white: Option<Entry>,
    entries: HashMap<(String, bool), Entry>,
    /// Files that failed to decode, with the time they had: tried again when the file changes.
    failed: HashMap<(String, bool), Option<SystemTime>>,
    warned: HashSet<String>,
    warnings: Vec<String>,
}

fn mtime_of(path: &str) -> Option<SystemTime> {
    std::fs::metadata(Path::new(path)).and_then(|m| m.modified()).ok()
}

/// The descriptor set layout every material pipeline uses: an image and a sampler.
pub fn set_layout(gpu: &Gpu) -> vk::DescriptorSetLayout {
    let mut guard = gpu.shared.textures.lock().unwrap();
    guard.get_or_insert_with(|| TextureCache::new(gpu)).layout
}

/// Runs f with the device's cache.
pub fn with_cache<R>(gpu: &Gpu, f: impl FnOnce(&mut TextureCache) -> R) -> R {
    let mut guard = gpu.shared.textures.lock().unwrap();
    f(guard.get_or_insert_with(|| TextureCache::new(gpu)))
}

impl TextureCache {
    fn new(gpu: &Gpu) -> TextureCache {
        unsafe {
            let d = &gpu.device;
            let bindings = [
                vk::DescriptorSetLayoutBinding::default().binding(0).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                vk::DescriptorSetLayoutBinding::default().binding(1).descriptor_type(vk::DescriptorType::SAMPLER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
            ];
            let layout = d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings), None).expect("texture layout");
            let sizes = [
                vk::DescriptorPoolSize::default().ty(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1024),
                vk::DescriptorPoolSize::default().ty(vk::DescriptorType::SAMPLER).descriptor_count(1024),
            ];
            let pool = d
                .create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET).max_sets(1024).pool_sizes(&sizes), None)
                .expect("texture pool");
            let sampler = d
                .create_sampler(
                    &vk::SamplerCreateInfo::default()
                        .mag_filter(vk::Filter::LINEAR)
                        .min_filter(vk::Filter::LINEAR)
                        .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
                        .address_mode_u(vk::SamplerAddressMode::REPEAT)
                        .address_mode_v(vk::SamplerAddressMode::REPEAT)
                        .address_mode_w(vk::SamplerAddressMode::REPEAT)
                        .max_lod(vk::LOD_CLAMP_NONE),
                    None,
                )
                .expect("sampler");
            let commands = d
                .create_command_pool(&vk::CommandPoolCreateInfo::default().queue_family_index(gpu.shared.queue_family).flags(vk::CommandPoolCreateFlags::TRANSIENT), None)
                .expect("texture commands");
            TextureCache { layout, pool, sampler, commands, white: None, entries: HashMap::new(), failed: HashMap::new(), warned: HashSet::new(), warnings: Vec::new() }
        }
    }

    /// Lets the memory go, as the device closes.
    pub fn release(&mut self, device: &ash::Device, allocator: &std::sync::Mutex<gpu_allocator::vulkan::Allocator>) {
        unsafe {
            let _ = device.device_wait_idle();
            for mut e in self.entries.drain().map(|x| x.1).chain(self.white.take()) {
                device.destroy_image_view(e.view, None);
                device.destroy_image(e.image, None);
                if let Some(a) = e.alloc.take() {
                    let _ = allocator.lock().unwrap().free(a);
                }
            }
        }
    }

    /// The white texel's set.
    pub fn white(&mut self, gpu: &Gpu) -> vk::DescriptorSet {
        if self.white.is_none() {
            let d = Decoded { width: 1, height: 1, rgba: vec![255; 4], has_alpha: false };
            self.white = self.upload(gpu, &d, false, None);
        }
        self.white.as_ref().map_or(vk::DescriptorSet::null(), |e| e.set)
    }

    /// A file's texture: loaded on first use, reloaded when it changes on disk. None when it can't be read.
    pub fn get(&mut self, gpu: &Gpu, path: &str, srgb: bool) -> Option<Loaded> {
        let key = (path.to_string(), srgb);
        if let Some(e) = self.entries.get_mut(&key) {
            if e.checked.elapsed() < Duration::from_secs(1) {
                return Some(Loaded { set: e.set, has_alpha: e.has_alpha, size: e.size });
            }
            e.checked = Instant::now();
            if mtime_of(path) == e.mtime {
                return Some(Loaded { set: e.set, has_alpha: e.has_alpha, size: e.size });
            }
            let old = self.entries.remove(&key).unwrap();
            self.destroy(gpu, old);
        }
        let mtime = mtime_of(path);
        if self.failed.get(&key).is_some_and(|t| *t == mtime) {
            return None;
        }
        let decoded = match decode(path) {
            Ok(d) => d,
            Err(e) => {
                self.failed.insert(key, mtime);
                self.warn(format!("Cannot read texture {path}: {e}"));
                return None;
            }
        };
        self.failed.remove(&key);
        let entry = self.upload(gpu, &decoded, srgb, mtime)?;
        let loaded = Loaded { set: entry.set, has_alpha: entry.has_alpha, size: entry.size };
        self.entries.insert(key, entry);
        Some(loaded)
    }

    /// Says something once in the script history, however often it holds.
    pub fn warn(&mut self, text: String) {
        if self.warned.insert(text.clone()) {
            self.warnings.push(text);
        }
    }

    pub fn take_warnings(&mut self) -> Vec<String> {
        std::mem::take(&mut self.warnings)
    }

    fn destroy(&mut self, gpu: &Gpu, mut e: Entry) {
        unsafe {
            // A frame in flight may still sample it.
            let _ = gpu.device.queue_wait_idle(gpu.queue);
            let _ = gpu.device.free_descriptor_sets(self.pool, &[e.set]);
            gpu.device.destroy_image_view(e.view, None);
            gpu.device.destroy_image(e.image, None);
            if let Some(a) = e.alloc.take() {
                let _ = gpu.shared.allocator.lock().unwrap().free(a);
            }
        }
    }

    fn upload(&mut self, gpu: &Gpu, source: &Decoded, srgb: bool, mtime: Option<SystemTime>) -> Option<Entry> {
        let limit = unsafe { gpu.shared.instance.get_physical_device_properties(gpu.shared.phys).limits.max_image_dimension2_d };
        // Bigger than the device takes: scaled to fit.
        let scaled;
        let d = if source.width > limit || source.height > limit {
            let k = limit as f64 / source.width.max(source.height) as f64;
            let (w, h) = (((source.width as f64 * k) as u32).max(1), ((source.height as f64 * k) as u32).max(1));
            let img = image::RgbaImage::from_raw(source.width, source.height, source.rgba.clone())?;
            let small = image::imageops::thumbnail(&img, w, h);
            scaled = Decoded { width: w, height: h, rgba: small.into_raw(), has_alpha: source.has_alpha };
            &scaled
        } else {
            source
        };
        let levels = 32 - d.width.max(d.height).leading_zeros();
        let format = if srgb { vk::Format::R8G8B8A8_SRGB } else { vk::Format::R8G8B8A8_UNORM };
        unsafe {
            let dev = &gpu.device;
            let image = dev
                .create_image(
                    &vk::ImageCreateInfo::default()
                        .image_type(vk::ImageType::TYPE_2D)
                        .format(format)
                        .extent(vk::Extent3D { width: d.width, height: d.height, depth: 1 })
                        .mip_levels(levels)
                        .array_layers(1)
                        .samples(vk::SampleCountFlags::TYPE_1)
                        .usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::TRANSFER_SRC),
                    None,
                )
                .ok()?;
            let req = dev.get_image_memory_requirements(image);
            let alloc = gpu.shared.allocator.lock().unwrap().allocate(&AllocationCreateDesc { name: "texture", requirements: req, location: MemoryLocation::GpuOnly, linear: false, allocation_scheme: AllocationScheme::GpuAllocatorManaged }).ok()?;
            dev.bind_image_memory(image, alloc.memory(), alloc.offset()).ok()?;
            let view = dev
                .create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(format)
                        .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(levels).layer_count(1)),
                    None,
                )
                .ok()?;

            let staging = gpu.buffer_at(vk::BufferUsageFlags::TRANSFER_SRC, &d.rgba, MemoryLocation::CpuToGpu);
            let cmd = dev.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(self.commands).level(vk::CommandBufferLevel::PRIMARY).command_buffer_count(1)).ok()?[0];
            dev.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)).ok()?;
            let range = |base: u32, count: u32| vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).base_mip_level(base).level_count(count).layer_count(1);
            let barrier = |r, old, new, src, dst| vk::ImageMemoryBarrier::default().image(image).subresource_range(r).old_layout(old).new_layout(new).src_access_mask(src).dst_access_mask(dst);
            dev.cmd_pipeline_barrier(cmd, vk::PipelineStageFlags::TOP_OF_PIPE, vk::PipelineStageFlags::TRANSFER, vk::DependencyFlags::empty(), &[], &[], &[barrier(range(0, levels), vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::AccessFlags::empty(), vk::AccessFlags::TRANSFER_WRITE)]);
            dev.cmd_copy_buffer_to_image(
                cmd,
                staging.buffer,
                image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[vk::BufferImageCopy::default().image_subresource(vk::ImageSubresourceLayers::default().aspect_mask(vk::ImageAspectFlags::COLOR).layer_count(1)).image_extent(vk::Extent3D { width: d.width, height: d.height, depth: 1 })],
            );
            let (mut w, mut h) = (d.width as i32, d.height as i32);
            for level in 1..levels {
                dev.cmd_pipeline_barrier(cmd, vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::TRANSFER, vk::DependencyFlags::empty(), &[], &[], &[barrier(range(level - 1, 1), vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, vk::AccessFlags::TRANSFER_WRITE, vk::AccessFlags::TRANSFER_READ)]);
                let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
                let layers = |m: u32| vk::ImageSubresourceLayers::default().aspect_mask(vk::ImageAspectFlags::COLOR).mip_level(m).layer_count(1);
                let blit = vk::ImageBlit::default()
                    .src_subresource(layers(level - 1))
                    .src_offsets([vk::Offset3D::default(), vk::Offset3D { x: w, y: h, z: 1 }])
                    .dst_subresource(layers(level))
                    .dst_offsets([vk::Offset3D::default(), vk::Offset3D { x: nw, y: nh, z: 1 }]);
                dev.cmd_blit_image(cmd, image, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, image, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[blit], vk::Filter::LINEAR);
                (w, h) = (nw, nh);
            }
            let mut last = vec![barrier(range(levels - 1, 1), vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::AccessFlags::TRANSFER_WRITE, vk::AccessFlags::SHADER_READ)];
            if levels > 1 {
                last.push(barrier(range(0, levels - 1), vk::ImageLayout::TRANSFER_SRC_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::AccessFlags::TRANSFER_READ, vk::AccessFlags::SHADER_READ));
            }
            dev.cmd_pipeline_barrier(cmd, vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::FRAGMENT_SHADER, vk::DependencyFlags::empty(), &[], &[], &last);
            dev.end_command_buffer(cmd).ok()?;
            let cmds = [cmd];
            dev.queue_submit(gpu.queue, &[vk::SubmitInfo::default().command_buffers(&cmds)], vk::Fence::null()).ok()?;
            dev.queue_wait_idle(gpu.queue).ok()?;
            dev.free_command_buffers(self.commands, &cmds);
            let mut staging = staging;
            dev.destroy_buffer(staging.buffer, None);
            if let Some(a) = staging.alloc.take() {
                let _ = gpu.shared.allocator.lock().unwrap().free(a);
            }

            let layouts = [self.layout];
            let set = dev.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(self.pool).set_layouts(&layouts)).ok()?[0];
            let image_info = [vk::DescriptorImageInfo::default().image_view(view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
            let sampler_info = [vk::DescriptorImageInfo::default().sampler(self.sampler)];
            let writes = [
                vk::WriteDescriptorSet::default().dst_set(set).dst_binding(0).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&image_info),
                vk::WriteDescriptorSet::default().dst_set(set).dst_binding(1).descriptor_type(vk::DescriptorType::SAMPLER).image_info(&sampler_info),
            ];
            dev.update_descriptor_sets(&writes, &[]);
            Some(Entry { image, view, alloc: Some(alloc), set, mtime, checked: Instant::now(), has_alpha: source.has_alpha, size: (source.width, source.height) })
        }
    }
}
