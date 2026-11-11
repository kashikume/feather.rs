use std::collections::HashMap;
use std::time::Instant;

use anyhow::{anyhow, Result};
use vulkanalia::loader::{LibloadingLoader, LIBRARY};
use vulkanalia::prelude::v1_0::*;
use vulkanalia::window as vk_window;
use winit::event::{ElementState, Event, KeyEvent, WindowEvent};
use winit::keyboard::PhysicalKey;
use winit::event_loop::EventLoop;
use winit::window::Window;

use vulkanalia::vk::ExtDebugUtilsExtension;
use vulkanalia::vk::KhrSurfaceExtension;
use vulkanalia::vk::KhrSwapchainExtension;

/// Whether the validation layers should be enabled.
const VALIDATION_ENABLED: bool = cfg!(debug_assertions);

/// The maximum number of frames that can be processed concurrently.
const MAX_FRAMES_IN_FLIGHT: usize = 2;

use super::appdata::AppData;
use super::buffers::FrameUniformBuffer;
use super::colorobjects::create_color_objects;
use super::commandbuffers::{create_command_buffers, record_command_buffer, GpuDraw};
use super::commandpool::create_command_pool;
use super::dephobjects::create_depth_objects;
use super::featherapp::FeatherApp;
use super::object::Object;
use super::framebuffers::create_framebuffers;
use super::instance::create_instance;
use super::logicaldevice::create_logical_device;
use super::physicaldevice::pick_physical_device;
use super::pipeline::{create_render_pass, PipelineKey, PipelineTarget, ShaderPipeline};
use super::scene::sort_for_rendering;
use super::swapchain::Swapchain;
use super::syncobjects::create_sync_objects;
use super::gpuimage::{create_texture_sampler, GpuImage, UploadContext};
use super::gpumeshbuffer::GpuMeshBuffer;
use super::shadowpass::{ShadowDraw, ShadowMap, ShadowPass, ShadowRenderer};
use super::uniformdata::{
    engine_uniforms, pack_draw, sampler_sources, SamplerSource, UniformArena, UniformValues,
};
use super::math::Mat4;
use cgmath::SquareMatrix;

/// Our Vulkan app.
pub struct App {
    _entry: Entry,
    instance: Instance,
    pub data: AppData,
    device: Device,
    frame: usize,
    pub resized: bool,
    start: Instant,
}

impl App {
    /// Creates our Vulkan app.
    pub unsafe fn create(window: &Window, app: Box<dyn FeatherApp>) -> Result<Self> {
        let loader = LibloadingLoader::new(LIBRARY)?;
        let entry = Entry::new(loader).map_err(|b| anyhow!("{}", b))?;
        let mut data = AppData {
            app,
            messenger: vk::DebugUtilsMessengerEXT::default(),
            surface: vk::SurfaceKHR::default(),
            physical_device: vk::PhysicalDevice::default(),
            msaa_samples: vk::SampleCountFlags::default(),
            graphics_queue: vk::Queue::default(),
            present_queue: vk::Queue::default(),
            swapchain: Swapchain::default(),
            render_pass: vk::RenderPass::default(),
            min_uniform_alignment: 1,
            framebuffers: Vec::new(),
            command_pool: vk::CommandPool::default(),
            color_image: vk::Image::default(),
            color_image_memory: vk::DeviceMemory::default(),
            color_image_view: vk::ImageView::default(),
            depth_image: vk::Image::default(),
            depth_image_memory: vk::DeviceMemory::default(),
            depth_image_view: vk::ImageView::default(),
            scenes: HashMap::new(),
            texture_sampler: vk::Sampler::default(),
            fallback_texture: None,
            shadow_renderer: None,
            fallback_shadow_map: None,
            frame_uniforms: Vec::new(),
            command_buffers: Vec::new(),
            image_available_semaphores: Vec::new(),
            render_finished_semaphores: Vec::new(),
            in_flight_fences: Vec::new(),
            images_in_flight: Vec::new(),
        };
        let instance = create_instance(window, &entry, &mut data)?;
        data.surface = vk_window::create_surface(&instance, &window, &window)?;
        pick_physical_device(&instance, &mut data)?;
        data.min_uniform_alignment = instance
            .get_physical_device_properties(data.physical_device)
            .limits
            .min_uniform_buffer_offset_alignment;
        let device = create_logical_device(&entry, &instance, &mut data)?;
        data.swapchain = Swapchain::create(window, &instance, &device, &data)?;
        data.swapchain.create_image_views(&device)?;
        create_render_pass(&instance, &device, &mut data)?;
        create_command_pool(&instance, &device, &mut data)?;
        create_color_objects(&instance, &device, &mut data)?;
        create_depth_objects(&instance, &device, &mut data)?;
        create_framebuffers(&device, &mut data)?;
        data.texture_sampler = create_texture_sampler(&device)?;
        let upload = UploadContext {
            instance: &instance,
            device: &device,
            physical_device: data.physical_device,
            command_pool: data.command_pool,
            queue: data.graphics_queue,
        };
        data.fallback_texture = Some(GpuImage::upload_rgba(&upload, 1, 1, &[255, 0, 255, 255])?);
        data.fallback_shadow_map = Some(GpuImage::upload_rgba(&upload, 1, 1, &[255, 255, 255, 255])?);
        data.shadow_renderer = Some(ShadowRenderer::create(&instance, &device, data.physical_device)?);
        data.app.on_create()?;
        create_command_buffers(&device, &mut data)?;
        data.frame_uniforms.resize_with(data.swapchain.swapchain_images.len(), || None);
        create_sync_objects(&device, &mut data)?;
        Ok(Self {
            _entry: entry,
            instance,
            data,
            device,
            frame: 0,
            resized: false,
            start: Instant::now(),
        })
    }

    /// Gets the scenes ready for drawing: advances their animations to `time` (the time passed
    /// to `on_update`), places new meshes in mesh buffers and updates node uniforms and
    /// transforms.
    fn prepare_scenes(&mut self, time: f32) -> Result<()> {
        for i in 0..self.data.app.get_num_scenes_to_render() {
            let scene = self.data.app.get_scene_to_render(i);
            scene.advance_animations_to(time);
            scene.build_missing_mesh_buffers()?;
            scene.update_nodes();
        }
        Ok(())
    }

    /// Releases the GPU state of scenes that are no longer rendered: mesh buffers, atlases and
    /// pipelines (with their descriptor sets). Waits for the GPU first if there are any.
    unsafe fn release_unrendered_scenes(&mut self, rendered: &[u64]) -> Result<()> {
        let gone: Vec<u64> = self
            .data
            .scenes
            .keys()
            .copied()
            .filter(|id| !rendered.contains(id))
            .collect();
        if gone.is_empty() {
            return Ok(());
        }
        self.device.device_wait_idle()?;
        for id in gone {
            self.data.scenes.remove(&id).unwrap().destroy(&self.device);
        }
        Ok(())
    }

    /// Collects the draw calls of every rendered scene, scene after scene, each in drawing
    /// order (`sort_for_rendering`: opaque by shader, then transparent back to front) and seen
    /// through its own camera, and packs their uniforms into the returned arena; and the shadow
    /// passes of the scenes with a shadow light. The first draw of every scene but the first
    /// clears the depth buffer, so later scenes are drawn over earlier ones. Creates the
    /// pipelines of shaders drawn for the first time.
    unsafe fn collect_draws(&mut self) -> Result<(Vec<GpuDraw>, Vec<ShadowPass>, UniformArena)> {
        let mut arena = UniformArena::new(self.data.min_uniform_alignment as usize);
        let mut draws = Vec::new();
        let mut shadow_passes = Vec::new();
        for index in 0..self.data.app.get_num_scenes_to_render() {
            let first_draw = draws.len();
            self.collect_scene_draws(index, &mut draws, &mut shadow_passes, &mut arena)?;
            if index > 0 {
                if let Some(draw) = draws.get_mut(first_draw) {
                    draw.clear_depth = true;
                }
            }
        }
        Ok((draws, shadow_passes, arena))
    }

    /// Adds the draws of scene `index` to `draws`, and its shadow pass to `shadow_passes` (see
    /// `collect_draws`).
    unsafe fn collect_scene_draws(
        &mut self,
        index: usize,
        draws: &mut Vec<GpuDraw>,
        shadow_passes: &mut Vec<ShadowPass>,
        arena: &mut UniformArena,
    ) -> Result<()> {
        let extent = self.data.swapchain.swapchain_extent;
        let camera = self.data.app.get_camera_to_render_scene(index);
        camera.set_screen_dimention(extent.width, extent.height);
        let (view, proj) = (camera.get_view(), camera.get_projection());

        let data = &mut self.data;
        let scene = data.app.get_scene_to_render(index);
        let scene_id = scene.get_id();
        let gpu = data.scenes.entry(scene_id).or_default();

        // Pipelines of shaders that no longer exist. They may still be used by frames in
        // flight, so wait for the GPU before destroying them.
        let removed: Vec<PipelineKey> = gpu
            .shader_pipelines
            .keys()
            .copied()
            .filter(|key| scene.get_shader(key.shader).is_none())
            .collect();
        if !removed.is_empty() {
            self.device.device_wait_idle()?;
            for key in removed {
                gpu.shader_pipelines.remove(&key).unwrap().destroy(&self.device);
            }
        }
        gpu.failed_pipelines
            .retain(|key| scene.get_shader(key.shader).is_some());

        // Opaque nodes first, by shader; then transparent ones, back to front.
        let mut items = scene.draw_items();
        sort_for_rendering(&mut items, &view);

        let target = PipelineTarget {
            render_pass: data.render_pass,
            msaa_samples: data.msaa_samples,
            extent,
        };
        let fallback_view = data.fallback_texture.as_ref().unwrap().view;

        // Shadows: the casters, drawn from the light into the scene's shadow map. Shaders
        // sample the map (or a map lit everywhere) with the shadow sampler.
        let light = scene.get_shadow_light().map(|l| l.view_projection);
        let shadow_matrix = light.unwrap_or_else(Mat4::identity);
        if let (Some(light), Some(map)) = (light, &gpu.shadow_map) {
            let casters = items
                .iter()
                .filter(|item| item.casts_shadow)
                .filter_map(|item| {
                    let buffer = gpu.gpu_mesh_buffers.get(&item.mesh_buffer)?;
                    Some(ShadowDraw {
                        model: item.model,
                        vertex_buffer: buffer.vertex_buffer,
                        index_buffer: buffer.index_buffer,
                        first_index: item.first_index,
                        index_count: item.index_count,
                        vertex_offset: item.vertex_offset,
                    })
                })
                .collect();
            shadow_passes.push(ShadowPass {
                framebuffer: map.framebuffer,
                size: map.size,
                light,
                draws: casters,
            });
        }
        let shadow_view = gpu
            .shadow_map
            .as_ref()
            .map_or(data.fallback_shadow_map.as_ref().unwrap().view, |m| m.view);
        let shadow_sampler = data.shadow_renderer.as_ref().unwrap().sampler;

        draws.reserve(items.len());
        let mut issues = Vec::new();
        for item in items {
            let key = PipelineKey {
                shader: item.shader,
                transparent: item.transparent,
            };
            if gpu.failed_pipelines.contains(&key) {
                continue;
            }
            let shader = scene.get_shader(item.shader).unwrap();
            if !gpu.shader_pipelines.contains_key(&key) {
                match ShaderPipeline::create(&self.device, &target, shader, key.transparent) {
                    Ok(pipeline) => {
                        gpu.shader_pipelines.insert(key, pipeline);
                    }
                    Err(e) => {
                        log::error!(
                            "Cannot draw with shader {}{}: {e:#}",
                            shader.get_name().unwrap_or_else(|| item.shader.to_string()),
                            if key.transparent { " (transparent)" } else { "" }
                        );
                        gpu.failed_pipelines.insert(key);
                        continue;
                    }
                }
            }
            let pipeline = &gpu.shader_pipelines[&key];
            let Some(buffer) = gpu.gpu_mesh_buffers.get(&item.mesh_buffer) else {
                continue;
            };
            let (vertex_buffer, index_buffer) = (buffer.vertex_buffer, buffer.index_buffer);
            let Some(node) = scene.get_node(item.node) else {
                continue;
            };

            // The material's texture: every sampler samples its atlas (or the fallback
            // texture), and its rectangle goes to the shader's umin/umax/vmin/vmax uniforms.
            let texture = scene
                .get_material(item.material)
                .and_then(|m| m.get_texture());
            issues.clear();
            let sources = sampler_sources(
                shader.get_layout(),
                shader.get_shadow_map_sampler_name(),
                texture,
                |a| scene.get_atlas(a).is_some(),
                &mut issues,
            );
            let textures = sources
                .iter()
                .map(|source| match source {
                    SamplerSource::ShadowMap => (shadow_view, shadow_sampler),
                    SamplerSource::Atlas(atlas) => (
                        atlas
                            .and_then(|a| gpu.gpu_atlases.get(&a))
                            .map_or(fallback_view, |image| image.view),
                        data.texture_sampler,
                    ),
                })
                .collect();

            // Uniforms: the node's dynamic uniforms, plus the matrices and the texture
            // rectangle the engine provides under the names the shader gave them.
            let engine = engine_uniforms(
                shader.get_uniform_names(),
                item.model,
                view,
                proj,
                shadow_matrix,
                texture,
            );
            let values = UniformValues {
                engine: &engine,
                node: node.dynamic_uniforms(),
            };
            let packed = pack_draw(shader.get_layout(), &values, arena, &mut issues);
            for issue in issues.drain(..) {
                if gpu.reported_uniform_issues.insert((item.shader, issue.clone())) {
                    log::warn!(
                        "Shader {}: {issue}",
                        shader.get_name().unwrap_or_else(|| item.shader.to_string())
                    );
                }
            }

            draws.push(GpuDraw {
                scene: scene_id,
                pipeline_key: key,
                clear_depth: false,
                textures,
                pipeline: pipeline.pipeline,
                pipeline_layout: pipeline.layout,
                descriptor_sets: Vec::new(),
                dynamic_offsets: packed.block_offsets,
                push_constants: packed.push_constants,
                push_constant_stages: pipeline.push_constant_stages,
                vertex_buffer,
                index_buffer,
                first_index: item.first_index,
                index_count: item.index_count,
                vertex_offset: item.vertex_offset,
            });
        }
        Ok(())
    }

    /// Copies the frame's uniform data to the uniform buffer of swapchain image `image_index`
    /// (growing it if needed) and points every pipeline's descriptor sets for that image at it.
    /// The image's previous frame must be finished.
    unsafe fn upload_uniforms(
        &mut self,
        image_index: usize,
        arena: &UniformArena,
        draws: &mut [GpuDraw],
    ) -> Result<()> {
        let needed = arena.bytes().len() as u64;
        let slot = &mut self.data.frame_uniforms[image_index];
        if slot.as_ref().map_or(true, |b| b.size < needed) {
            if let Some(old) = slot.take() {
                old.destroy(&self.device);
            }
            *slot = Some(FrameUniformBuffer::create(
                &self.instance,
                &self.device,
                &self.data.physical_device,
                needed,
            )?);
        }
        let buffer = slot.as_ref().unwrap();
        buffer.write(arena.bytes());

        for draw in draws {
            let pipeline = self
                .data
                .scenes
                .get_mut(&draw.scene)
                .and_then(|s| s.shader_pipelines.get_mut(&draw.pipeline_key))
                .unwrap();
            draw.descriptor_sets = pipeline.descriptor_sets(
                &self.device,
                image_index,
                &draw.textures,
                buffer.buffer,
            )?;
        }
        Ok(())
    }

    /// Brings the GPU state in line with the rendered scenes: releases everything of scenes no
    /// longer rendered; uploads new mesh buffers and atlases of each rendered scene, and
    /// releases removed ones.
    unsafe fn sync_gpu_state(&mut self) -> Result<()> {
        let count = self.data.app.get_num_scenes_to_render();
        let rendered: Vec<u64> = (0..count)
            .map(|i| self.data.app.get_scene_to_render(i).get_id())
            .collect();
        self.release_unrendered_scenes(&rendered)?;
        for index in 0..count {
            self.sync_scene(index)?;
        }
        Ok(())
    }

    /// Uploads new mesh buffers and atlases of scene `index` and releases removed ones.
    unsafe fn sync_scene(&mut self, index: usize) -> Result<()> {
        let data = &mut self.data;
        let scene = data.app.get_scene_to_render(index);
        let gpu = data.scenes.entry(scene.get_id()).or_default();

        let upload = UploadContext {
            instance: &self.instance,
            device: &self.device,
            physical_device: data.physical_device,
            command_pool: data.command_pool,
            queue: data.graphics_queue,
        };

        // Mesh buffers. Their contents never change once created. A removed buffer's handle
        // may already belong to a new buffer: compare ids.
        let removed: Vec<usize> = gpu
            .gpu_mesh_buffers
            .iter()
            .filter(|(&h, b)| scene.buffers.get(h).map(|b| b.get_id()) != Some(b.id))
            .map(|(&h, _)| h)
            .collect();
        if !removed.is_empty() {
            self.device.device_wait_idle()?;
            for handle in removed {
                gpu.gpu_mesh_buffers.remove(&handle).unwrap().destroy(&self.device);
            }
        }
        for buffer in scene.buffers.iter() {
            let handle = buffer.get_handle();
            if !gpu.gpu_mesh_buffers.contains_key(&handle) {
                let gpu_buffer = GpuMeshBuffer::upload(&upload, buffer, &scene.meshes)?;
                gpu.gpu_mesh_buffers.insert(handle, gpu_buffer);
            }
        }

        // Atlases.
        let removed: Vec<usize> = gpu
            .gpu_atlases
            .keys()
            .copied()
            .filter(|&h| scene.get_atlas(h).is_none())
            .collect();
        if !removed.is_empty() {
            // Frames in flight and cached descriptor sets may use the images. Pipelines,
            // with their descriptor sets, are recreated when next needed.
            self.device.device_wait_idle()?;
            gpu.shader_pipelines
                .drain()
                .for_each(|(_, p)| p.destroy(&self.device));
            for handle in removed {
                gpu.gpu_atlases.remove(&handle).unwrap().destroy(&self.device);
            }
        }
        gpu.failed_atlases.retain(|&h| scene.get_atlas(h).is_some());

        // The shadow map, as big as the scene's shadow light wants it.
        let wanted = scene.get_shadow_light().map(|l| l.map_size);
        if gpu.shadow_map.as_ref().map(|m| m.size) != wanted {
            if let Some(old) = gpu.shadow_map.take() {
                // Frames in flight and cached descriptor sets may use it.
                self.device.device_wait_idle()?;
                gpu.shader_pipelines
                    .drain()
                    .for_each(|(_, p)| p.destroy(&self.device));
                old.destroy(&self.device);
            }
            if let Some(size) = wanted {
                gpu.shadow_map = Some(ShadowMap::create(
                    &self.instance,
                    &self.device,
                    data.physical_device,
                    data.shadow_renderer.as_ref().unwrap(),
                    size,
                )?);
            }
        }

        for atlas in scene.atlases.iter() {
            let handle = atlas.get_handle();
            if gpu.gpu_atlases.contains_key(&handle) || gpu.failed_atlases.contains(&handle) {
                continue;
            }
            match GpuImage::upload_rgba(
                &upload,
                atlas.get_width(),
                atlas.get_height(),
                atlas.get_pixels(),
            ) {
                Ok(image) => {
                    gpu.gpu_atlases.insert(handle, image);
                }
                Err(e) => {
                    log::error!(
                        "Cannot upload atlas {}: {e:#}",
                        atlas.get_name().unwrap_or_else(|| handle.to_string())
                    );
                    gpu.failed_atlases.insert(handle);
                }
            }
        }
        Ok(())
    }

    /// Renders a frame for our Vulkan app.
    pub unsafe fn render(&mut self, window: &Window) -> Result<()> {
        let time = self.start.elapsed().as_secs_f32();
        self.data.app.on_update(time)?;
        self.prepare_scenes(time)?;
        self.sync_gpu_state()?;
        self.data.app.on_render()?;
        let in_flight_fence = self.data.in_flight_fences[self.frame];

        self.device
            .wait_for_fences(&[in_flight_fence], true, u64::MAX)?;

        let result = self.device.acquire_next_image_khr(
            self.data.swapchain.swapchain,
            u64::MAX,
            self.data.image_available_semaphores[self.frame],
            vk::Fence::null(),
        );

        let image_index = match result {
            Ok((image_index, _)) => image_index as usize,
            Err(vk::ErrorCode::OUT_OF_DATE_KHR) => return self.recreate_swapchain(window),
            Err(e) => return Err(anyhow!(e)),
        };

        let image_in_flight = self.data.images_in_flight[image_index];
        if !image_in_flight.is_null() {
            self.device
                .wait_for_fences(&[image_in_flight], true, u64::MAX)?;
        }

        self.data.images_in_flight[image_index] = in_flight_fence;

        let (mut draws, shadow_passes, arena) = self.collect_draws()?;
        self.upload_uniforms(image_index, &arena, &mut draws)?;
        record_command_buffer(&self.device, &self.data, image_index, &shadow_passes, &draws)?;

        let wait_semaphores = &[self.data.image_available_semaphores[self.frame]];
        let wait_stages = &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let command_buffers = &[self.data.command_buffers[image_index]];
        let signal_semaphores = &[self.data.render_finished_semaphores[self.frame]];
        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(wait_semaphores)
            .wait_dst_stage_mask(wait_stages)
            .command_buffers(command_buffers)
            .signal_semaphores(signal_semaphores);

        self.device.reset_fences(&[in_flight_fence])?;

        self.device
            .queue_submit(self.data.graphics_queue, &[submit_info], in_flight_fence)?;

        let swapchains = &[self.data.swapchain.swapchain];
        let image_indices = &[image_index as u32];
        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(signal_semaphores)
            .swapchains(swapchains)
            .image_indices(image_indices);

        let result = self
            .device
            .queue_present_khr(self.data.present_queue, &present_info);
        let changed = result == Ok(vk::SuccessCode::SUBOPTIMAL_KHR)
            || result == Err(vk::ErrorCode::OUT_OF_DATE_KHR);
        if self.resized || changed {
            self.resized = false;
            self.recreate_swapchain(window)?;
        } else if let Err(e) = result {
            return Err(anyhow!(e));
        }

        self.frame = (self.frame + 1) % MAX_FRAMES_IN_FLIGHT;

        Ok(())
    }

    /// Recreates the swapchain for our Vulkan app.
    unsafe fn recreate_swapchain(&mut self, window: &Window) -> Result<()> {
        self.device.device_wait_idle()?;
        self.destroy_swapchain();
        self.data.swapchain = Swapchain::create(window, &self.instance, &self.device, &self.data)?;
        self.data.swapchain.create_image_views(&self.device)?;
        create_render_pass(&self.instance, &self.device, &mut self.data)?;
        create_color_objects(&self.instance, &self.device, &mut self.data)?;
        create_depth_objects(&self.instance, &self.device, &mut self.data)?;
        create_framebuffers(&self.device, &mut self.data)?;
        create_command_buffers(&self.device, &mut self.data)?;
        self.data.frame_uniforms.resize_with(self.data.swapchain.swapchain_images.len(), || None);
        self.data.images_in_flight.resize(
            self.data.swapchain.swapchain_images.len(),
            vk::Fence::null(),
        );
        Ok(())
    }

    /// Destroys our Vulkan app.
    #[rustfmt::skip]
    pub unsafe fn destroy(&mut self) {
        self.data.app.on_destroy();
        self.device.device_wait_idle().unwrap();

        self.destroy_swapchain();

        self.data.in_flight_fences.iter().for_each(|f| self.device.destroy_fence(*f, None));
        self.data.render_finished_semaphores.iter().for_each(|s| self.device.destroy_semaphore(*s, None));
        self.data.image_available_semaphores.iter().for_each(|s| self.device.destroy_semaphore(*s, None));
        self.data.scenes.drain().for_each(|(_, mut s)| s.destroy(&self.device));
        if let Some(fallback) = self.data.fallback_texture.take() {
            fallback.destroy(&self.device);
        }
        if let Some(fallback) = self.data.fallback_shadow_map.take() {
            fallback.destroy(&self.device);
        }
        if let Some(renderer) = self.data.shadow_renderer.take() {
            renderer.destroy(&self.device);
        }
        self.device.destroy_sampler(self.data.texture_sampler, None);
        self.device.destroy_command_pool(self.data.command_pool, None);
        self.device.destroy_device(None);
        self.instance.destroy_surface_khr(self.data.surface, None);

        if VALIDATION_ENABLED {
            self.instance.destroy_debug_utils_messenger_ext(self.data.messenger, None);
        }

        self.instance.destroy_instance(None);
    }

    /// Destroys the parts of our Vulkan app related to the swapchain.
    #[rustfmt::skip]
    unsafe fn destroy_swapchain(&mut self) {
        self.device.free_command_buffers(self.data.command_pool, &self.data.command_buffers);
        self.data.command_buffers.clear();
        self.data.frame_uniforms.drain(..).flatten().for_each(|b| b.destroy(&self.device));
        self.device.destroy_image_view(self.data.depth_image_view, None);
        self.device.free_memory(self.data.depth_image_memory, None);
        self.device.destroy_image(self.data.depth_image, None);
        self.device.destroy_image_view(self.data.color_image_view, None);
        self.device.free_memory(self.data.color_image_memory, None);
        self.device.destroy_image(self.data.color_image, None);
        self.data.framebuffers.iter().for_each(|f| self.device.destroy_framebuffer(*f, None));
        // Pipelines depend on the render pass and the extent; they are recreated when needed.
        for scene in self.data.scenes.values_mut() {
            scene.destroy_pipelines(&self.device);
        }
        self.device.destroy_render_pass(self.data.render_pass, None);
        self.data.swapchain.destroy(&self.device);
    }

    /// Sets the window title from the app, when it has one (`FeatherApp::window_title`).
    fn apply_window_title(&self, window: &Window) {
        if let Some(title) = self.data.app.window_title() {
            window.set_title(&title);
        }
    }

    pub fn run(&mut self, window: &Window, event_loop: EventLoop<()>) -> Result<()> {
        let mut minimized = false;
        self.apply_window_title(window);
        event_loop.run(move |event, elwt| {
            match event {
                // Request a redraw when all events were processed.
                Event::AboutToWait => window.request_redraw(),
                Event::WindowEvent { event, .. } => match event {
                    // Render a frame if our Vulkan app is not being destroyed.
                    WindowEvent::RedrawRequested if !elwt.exiting() && !minimized => {
                        unsafe { self.render(&window) }.unwrap();
                    }
                    // Mark the window as having been resized.
                    WindowEvent::Resized(size) => {
                        if size.width == 0 || size.height == 0 {
                            minimized = true;
                        } else {
                            minimized = false;
                            self.resized = true;
                        }
                    }
                    // Pass key presses to the app.
                    WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                physical_key: PhysicalKey::Code(key),
                                state: ElementState::Pressed,
                                repeat: false,
                                ..
                            },
                        ..
                    } => {
                        if let Err(e) = self.data.app.on_key_pressed(key) {
                            log::error!("Handling key {key:?} failed: {e:#}");
                        }
                        self.apply_window_title(window);
                    }
                    // Destroy our Vulkan app.
                    WindowEvent::CloseRequested => {
                        elwt.exit();
                        unsafe {
                            self.destroy();
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        })?;
        Ok(())
    }
}
