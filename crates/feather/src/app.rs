use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use anyhow::{anyhow, bail, Result};
use vulkanalia::loader::{LibloadingLoader, LIBRARY};
use vulkanalia::prelude::v1_0::*;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, Touch, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowAttributes, WindowId};

use vulkanalia::vk::ExtDebugUtilsExtension;
use vulkanalia::vk::KhrSurfaceExtension;
use vulkanalia::vk::KhrSwapchainExtension;

/// Set by `request_exit`, checked by `App::run`'s loop.
static EXIT_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Asks `App::run` to end its loop, destroying the engine and the app, as closing the window
/// does. Can be called from any thread.
///
/// For Android, where nothing closes the window: when the activity is finished (back), winit
/// 0.30 doesn't end its loop, so the activity asks for it through this (a JNI call from its
/// back handler; see the test app's `android.rs`).
pub fn request_exit() {
    EXIT_REQUESTED.store(true, Ordering::SeqCst);
}

/// The maximum number of frames that can be processed concurrently.
const MAX_FRAMES_IN_FLIGHT: usize = 2;

use super::appdata::AppData;
use super::buffers::FrameUniformBuffer;
use super::colorobjects::create_color_objects;
use super::commandbuffers::{create_command_buffers, record_command_buffer, GpuDraw};
use super::commandpool::create_command_pool;
use super::dephobjects::create_depth_objects;
use super::featherapp::{FeatherApp, FrameStats, SceneStats, PIXELS_PER_WHEEL_LINE};
use super::framebuffers::create_framebuffers;
use super::gpuimage::{create_texture_sampler, GpuImage, UploadContext};
use super::gpumeshbuffer::GpuMeshBuffer;
use super::instance::create_instance;
use super::logicaldevice::create_logical_device;
use super::math::Mat4;
use super::object::Object;
use super::physicaldevice::{pick_physical_device, DeviceFeatures};
use super::pipeline::{create_render_pass, PipelineKey, PipelineTarget, ShaderPipeline};
use super::queuefamilyindices::QueueFamilyIndices;
use super::scene::{sort_for_rendering, Culling};
use super::shadowpass::{ShadowDraw, ShadowMap, ShadowPass, ShadowRenderer};
use super::surface;
use super::swapchain::Swapchain;
use super::syncobjects::create_sync_objects;
use super::touch::{MouseCall, TouchAsMouse};
use super::uniformdata::{
    engine_uniforms, pack_draw, sampler_sources, SamplerSource, UniformArena, UniformValues,
};
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
    /// The queue families the device was created with, for the surface it was created for.
    queue_families: QueueFamilyIndices,
    /// Whether the window's surface is gone (`suspend`): nothing is drawn until `resume`.
    suspended: bool,
    /// When the frame stats were last logged (at `debug` level, about once a second).
    stats_logged: Instant,
}

impl App {
    /// Creates our Vulkan app.
    pub unsafe fn create(window: &Window, app: Box<dyn FeatherApp>) -> Result<Self> {
        let loader = LibloadingLoader::new(LIBRARY)?;
        let entry = Entry::new(loader).map_err(|b| anyhow!("{}", b))?;
        let mut data = AppData {
            app,
            validation: false,
            messenger: vk::DebugUtilsMessengerEXT::default(),
            surface: vk::SurfaceKHR::default(),
            physical_device: vk::PhysicalDevice::default(),
            msaa_samples: vk::SampleCountFlags::default(),
            features: DeviceFeatures::default(),
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
        data.surface = surface::create_surface(&instance, window)?;
        pick_physical_device(&instance, &mut data)?;
        let queue_families =
            QueueFamilyIndices::get(&instance, data.surface, data.physical_device)?;
        data.min_uniform_alignment = instance
            .get_physical_device_properties(data.physical_device)
            .limits
            .min_uniform_buffer_offset_alignment;
        let device = create_logical_device(&entry, &instance, &mut data)?;
        data.swapchain = Swapchain::create(window, &instance, &device, &data, true)?;
        data.swapchain.create_image_views(&device)?;
        create_render_pass(&instance, &device, &mut data)?;
        create_command_pool(&instance, &device, &mut data)?;
        create_color_objects(&instance, &device, &mut data)?;
        create_depth_objects(&instance, &device, &mut data)?;
        create_framebuffers(&device, &mut data)?;
        data.texture_sampler = create_texture_sampler(&device, &data.features)?;
        let upload = UploadContext {
            instance: &instance,
            device: &device,
            physical_device: data.physical_device,
            command_pool: data.command_pool,
            queue: data.graphics_queue,
        };
        data.fallback_texture = Some(GpuImage::upload_rgba(&upload, 1, 1, &[255, 0, 255, 255])?);
        data.fallback_shadow_map =
            Some(GpuImage::upload_rgba(&upload, 1, 1, &[255, 255, 255, 255])?);
        data.shadow_renderer = Some(ShadowRenderer::create(
            &instance,
            &device,
            data.physical_device,
        )?);
        data.app.on_create()?;
        create_command_buffers(&device, &mut data)?;
        data.frame_uniforms
            .resize_with(data.swapchain.swapchain_images.len(), || None);
        create_sync_objects(&device, &mut data)?;
        Ok(Self {
            _entry: entry,
            instance,
            data,
            device,
            frame: 0,
            resized: false,
            start: Instant::now(),
            queue_families,
            suspended: false,
            stats_logged: Instant::now(),
        })
    }

    /// Drops the window's surface and the swapchain, with what depends on them (render pass,
    /// framebuffers, color and depth images, pipelines), and stops drawing. Everything else stays:
    /// the device, the scenes' meshes and atlases, the app.
    ///
    /// For winit's `Suspended`: on Android the window's native window is about to be destroyed
    /// (the app goes to the background, the screen is locked). Does nothing when already
    /// suspended.
    pub unsafe fn suspend(&mut self) -> Result<()> {
        if self.suspended {
            return Ok(());
        }
        log::info!("Suspended: releasing the surface and the swapchain.");
        self.device.device_wait_idle()?;
        self.destroy_swapchain();
        self.instance.destroy_surface_khr(self.data.surface, None);
        self.data.surface = vk::SurfaceKHR::null();
        self.suspended = true;
        Ok(())
    }

    /// Makes a new surface for `window` and a swapchain for it, after `suspend`, and tells the app
    /// the window's size. For winit's `Resumed` after a `Suspended`. Does nothing when not
    /// suspended.
    pub unsafe fn resume(&mut self, window: &Window) -> Result<()> {
        if !self.suspended {
            return Ok(());
        }
        log::info!("Resumed: recreating the surface and the swapchain.");
        self.data.surface = surface::create_surface(&self.instance, window)?;
        let present = self.queue_families.present;
        if !self.instance.get_physical_device_surface_support_khr(
            self.data.physical_device,
            present,
            self.data.surface,
        )? {
            bail!("the device's present queue (family {present}) can't present to the new surface");
        }
        self.create_swapchain_objects(window, true)?;
        self.suspended = false;
        self.resized = false;
        let size = window.inner_size();
        if size.width > 0 && size.height > 0 {
            if let Err(e) = self.data.app.on_resized(size.width, size.height) {
                log::error!("Handling the window size after resuming failed: {e:#}");
            }
        }
        Ok(())
    }

    /// Whether the surface is gone (between `suspend` and `resume`).
    pub fn is_suspended(&self) -> bool {
        self.suspended
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

    /// Passes the frame's stats to the app, and logs them at `debug` level about once a second,
    /// a line per scene.
    fn report_frame_stats(&mut self, stats: &FrameStats) {
        self.data.app.on_frame_stats(stats);
        if self.stats_logged.elapsed().as_secs_f32() >= 1.0 {
            self.stats_logged = Instant::now();
            log::debug!(
                "Frame: {} scenes collected in {:.2} ms",
                stats.scenes.len(),
                stats.collect_time * 1000.0
            );
            for (index, s) in stats.scenes.iter().enumerate() {
                log::debug!(
                    "  Scene {index}: {} draws, {} culled, {} shadow draws; {} nodes, {} meshes, \
                     {} mesh buffers, {} atlases",
                    s.draws,
                    s.culled,
                    s.shadow_draws,
                    s.nodes,
                    s.meshes,
                    s.mesh_buffers,
                    s.atlases
                );
            }
        }
    }

    /// Collects the draw calls of every rendered scene, scene after scene, each in drawing
    /// order (`sort_for_rendering`: opaque by shader, then transparent back to front) and seen
    /// through its active camera, and packs their uniforms into the returned arena; and the
    /// shadow passes of the scenes with a shadow light. Scenes without a camera are skipped. The
    /// first draw of every scene but the first clears the depth buffer, so later scenes are
    /// drawn over earlier ones. Creates the pipelines of shaders drawn for the first time.
    /// `stats` gets a `SceneStats` per scene (its `collect_time` is filled in by `render`).
    unsafe fn collect_draws(
        &mut self,
        stats: &mut FrameStats,
    ) -> Result<(Vec<GpuDraw>, Vec<ShadowPass>, UniformArena)> {
        let mut arena = UniformArena::new(self.data.min_uniform_alignment as usize);
        let mut draws = Vec::new();
        let mut shadow_passes = Vec::new();
        for index in 0..self.data.app.get_num_scenes_to_render() {
            let first_draw = draws.len();
            let mut scene_stats = SceneStats::default();
            self.collect_scene_draws(
                index,
                &mut draws,
                &mut shadow_passes,
                &mut arena,
                &mut scene_stats,
            )?;
            scene_stats.draws = draws.len() - first_draw;
            stats.scenes.push(scene_stats);
            if index > 0 {
                if let Some(draw) = draws.get_mut(first_draw) {
                    draw.clear_depth = true;
                }
            }
        }
        Ok((draws, shadow_passes, arena))
    }

    /// Adds the draws of scene `index` to `draws`, and its shadow pass to `shadow_passes` (see
    /// `collect_draws`). `stats` gets the scene's resources, culled nodes and shadow draws.
    unsafe fn collect_scene_draws(
        &mut self,
        index: usize,
        draws: &mut Vec<GpuDraw>,
        shadow_passes: &mut Vec<ShadowPass>,
        arena: &mut UniformArena,
        stats: &mut SceneStats,
    ) -> Result<()> {
        let extent = self.data.swapchain.swapchain_extent;
        let data = &mut self.data;
        let scene = data.app.get_scene_to_render(index);
        let scene_id = scene.get_id();
        stats.nodes = scene.nodes.len();
        stats.meshes = scene.meshes.len();
        stats.mesh_buffers = scene.buffers.len();
        stats.atlases = scene.atlases.len();
        let gpu = data.scenes.entry(scene_id).or_default();

        // Seen through the scene's active camera; a scene without a camera isn't drawn.
        let Some(camera) = scene.active_camera_mut() else {
            if !gpu.warned_no_camera {
                log::warn!("Scene {index} has no camera; it isn't drawn");
                gpu.warned_no_camera = true;
            }
            return Ok(());
        };
        gpu.warned_no_camera = false;
        camera.set_screen_dimention(extent.width, extent.height);
        let (view, proj) = (camera.get_view(), camera.get_projection());
        let scene = &*scene;

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
                gpu.shader_pipelines
                    .remove(&key)
                    .unwrap()
                    .destroy(&self.device);
            }
        }
        gpu.failed_pipelines
            .retain(|key| scene.get_shader(key.shader).is_some());

        // What may be seen: by the camera (the main pass) or by the shadow light (casters).
        let light = scene.get_shadow_light().map(|l| l.view_projection);
        let culling = Culling::new(&(proj * view), light.as_ref());
        let items = scene.draw_items(Some(&culling));
        stats.culled = culling.culled_count();

        let target = PipelineTarget {
            render_pass: data.render_pass,
            msaa_samples: data.msaa_samples,
            sample_shading: data.features.sample_rate_shading,
            extent,
        };
        let fallback_view = data.fallback_texture.as_ref().unwrap().view;

        // Shadows: the casters, drawn from the light into the scene's shadow map. Shaders
        // sample the map (or a map lit everywhere) with the shadow sampler.
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
                .collect::<Vec<_>>();
            stats.shadow_draws = casters.len();
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

        // The main pass: what the camera may see. Opaque nodes first, by shader; then
        // transparent ones, back to front.
        let mut items: Vec<_> = items.into_iter().filter(|item| item.in_view).collect();
        sort_for_rendering(&mut items, &view);

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
                            if key.transparent {
                                " (transparent)"
                            } else {
                                ""
                            }
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
                shader.get_uniform_ids().shadow_map,
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
                shader.get_uniform_ids(),
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
                if gpu
                    .reported_uniform_issues
                    .insert((item.shader, issue.clone()))
                {
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
            let old_handle = slot.as_ref().map(|b| b.buffer);
            if let Some(old) = slot.take() {
                old.destroy(&self.device);
            }
            let new = FrameUniformBuffer::create(
                &self.instance,
                &self.device,
                &self.data.physical_device,
                needed,
            )?;
            if let Some(old_handle) = old_handle {
                log::info!(
                    "Uniform buffer of swapchain image {image_index} grown to {} KB (handle {old_handle:?} -> {:?}).",
                    new.size / 1024,
                    new.buffer,
                );
            }
            *slot = Some(new);
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
                buffer.id,
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
                gpu.gpu_mesh_buffers
                    .remove(&handle)
                    .unwrap()
                    .destroy(&self.device);
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
                gpu.gpu_atlases
                    .remove(&handle)
                    .unwrap()
                    .destroy(&self.device);
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

        let collect_start = Instant::now();
        let mut stats = FrameStats::default();
        let (mut draws, shadow_passes, arena) = self.collect_draws(&mut stats)?;
        stats.collect_time = collect_start.elapsed().as_secs_f32();
        self.report_frame_stats(&stats);
        self.upload_uniforms(image_index, &arena, &mut draws)?;
        record_command_buffer(
            &self.device,
            &self.data,
            image_index,
            &shadow_passes,
            &draws,
        )?;

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
        // Suboptimal: the swapchain no longer matches the surface (e.g. a resize), unless it was
        // made that way on purpose (`Swapchain::suboptimal_expected`: Android in landscape).
        let suboptimal = result == Ok(vk::SuccessCode::SUBOPTIMAL_KHR)
            && !self.data.swapchain.suboptimal_expected;
        let changed = suboptimal || result == Err(vk::ErrorCode::OUT_OF_DATE_KHR);
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
        self.create_swapchain_objects(window, false)
    }

    /// Creates the swapchain for the current surface and what depends on it (the parts
    /// `destroy_swapchain` destroys). With `log`, logs the surface's capabilities.
    unsafe fn create_swapchain_objects(&mut self, window: &Window, log: bool) -> Result<()> {
        self.data.swapchain =
            Swapchain::create(window, &self.instance, &self.device, &self.data, log)?;
        self.data.swapchain.create_image_views(&self.device)?;
        create_render_pass(&self.instance, &self.device, &mut self.data)?;
        create_color_objects(&self.instance, &self.device, &mut self.data)?;
        create_depth_objects(&self.instance, &self.device, &mut self.data)?;
        create_framebuffers(&self.device, &mut self.data)?;
        create_command_buffers(&self.device, &mut self.data)?;
        self.data
            .frame_uniforms
            .resize_with(self.data.swapchain.swapchain_images.len(), || None);
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

        if !self.suspended {
            self.destroy_swapchain();
        }

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

        if self.data.validation {
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

    /// Runs `app` in a window made with `attributes`, until the window is closed (on Android:
    /// the activity is finished).
    ///
    /// The engine follows winit's lifecycle, which every platform goes through:
    ///
    /// - the first `resumed` makes the window, creates the engine (`App::create`) and calls the
    ///   app's `on_create`; on Android the window has no surface before it;
    /// - `suspended` drops the surface and the swapchain (`suspend`); a later `resumed` makes
    ///   them again (`resume`);
    /// - `exiting` destroys the engine and the app (`destroy`).
    ///
    /// In debug builds, F9 suspends and resumes at once, to exercise on Windows what Android
    /// does when the app goes to the background and back.
    ///
    /// An error creating, resuming or drawing ends the loop and is returned. Errors from the
    /// app's input and resize handlers are logged.
    pub fn run(
        event_loop: EventLoop<()>,
        attributes: WindowAttributes,
        app: Box<dyn FeatherApp>,
    ) -> Result<()> {
        let mut runner = Runner {
            attributes,
            window: None,
            pending: Some(app),
            engine: None,
            minimized: false,
            cursor: (0.0, 0.0),
            touch: TouchAsMouse::default(),
            error: None,
        };
        event_loop.run_app(&mut runner)?;
        match runner.error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

/// The state of `App::run`'s event loop.
struct Runner {
    /// What the window is made with, on the first `resumed`.
    attributes: WindowAttributes,
    window: Option<Window>,
    /// The app, until the engine is created on the first `resumed`.
    pending: Option<Box<dyn FeatherApp>>,
    engine: Option<App>,
    minimized: bool,
    /// Where the mouse pointer was last seen: button and wheel events don't say.
    cursor: (f32, f32),
    /// Touches turned into mouse calls (Android).
    touch: TouchAsMouse,
    /// The first error that ended the loop.
    error: Option<anyhow::Error>,
}

impl ApplicationHandler for Runner {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let result = self.on_resumed(event_loop);
        self.check(result, event_loop);
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(engine) = &mut self.engine {
            let result = unsafe { engine.suspend() };
            self.check(result, event_loop);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let result = self.on_window_event(event, event_loop);
        self.check(result, event_loop);
    }

    // Request a redraw when all events were processed.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if EXIT_REQUESTED.swap(false, Ordering::SeqCst) {
            log::info!("Exit requested.");
            event_loop.exit();
            return;
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn exiting(&mut self, _: &ActiveEventLoop) {
        if let Some(mut engine) = self.engine.take() {
            unsafe { engine.destroy() };
        }
    }
}

impl Runner {
    /// Ends the loop on the first error, keeping it for `App::run` to return.
    fn check(&mut self, result: Result<()>, event_loop: &ActiveEventLoop) {
        if let Err(e) = result {
            log::error!("{e:#}");
            self.error.get_or_insert(e);
            event_loop.exit();
        }
    }

    /// The first `resumed` makes the window and creates the engine; later ones (after
    /// `suspended`) bring back the surface.
    fn on_resumed(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        if self.window.is_none() {
            self.window = Some(event_loop.create_window(self.attributes.clone())?);
        }
        let window = self.window.as_ref().unwrap();
        if let Some(app) = self.pending.take() {
            let mut engine = unsafe { App::create(window, app)? };
            engine.apply_window_title(window);
            let size = window.inner_size();
            if size.width > 0 && size.height > 0 {
                engine.data.app.on_resized(size.width, size.height)?;
            }
            self.engine = Some(engine);
        } else if let Some(engine) = &mut self.engine {
            unsafe { engine.resume(window)? };
        }
        Ok(())
    }

    fn on_window_event(&mut self, event: WindowEvent, event_loop: &ActiveEventLoop) -> Result<()> {
        let (Some(engine), Some(window)) = (&mut self.engine, &self.window) else {
            return Ok(());
        };
        match event {
            // Render a frame if the engine is not being destroyed and has a surface.
            WindowEvent::RedrawRequested
                if !event_loop.exiting() && !self.minimized && !engine.is_suspended() =>
            {
                unsafe { engine.render(window)? };
            }
            // Mark the window as having been resized.
            WindowEvent::Resized(size) => {
                if size.width == 0 || size.height == 0 {
                    self.minimized = true;
                } else {
                    self.minimized = false;
                    engine.resized = true;
                    if let Err(e) = engine.data.app.on_resized(size.width, size.height) {
                        log::error!("Handling the window resize failed: {e:#}");
                    }
                }
            }
            // Pass the mouse to the app.
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as f32, position.y as f32);
                if let Err(e) = engine.data.app.on_mouse_moved(self.cursor.0, self.cursor.1) {
                    log::error!("Handling a mouse move failed: {e:#}");
                }
            }
            WindowEvent::CursorLeft { .. } => {
                if let Err(e) = engine.data.app.on_mouse_left() {
                    log::error!("Handling the mouse leaving failed: {e:#}");
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let pressed = state == ElementState::Pressed;
                let (x, y) = self.cursor;
                if let Err(e) = engine.data.app.on_mouse_button(button, pressed, x, y) {
                    log::error!("Handling mouse button {button:?} failed: {e:#}");
                }
                engine.apply_window_title(window);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x, y),
                    MouseScrollDelta::PixelDelta(p) => (
                        p.x as f32 / PIXELS_PER_WHEEL_LINE,
                        p.y as f32 / PIXELS_PER_WHEEL_LINE,
                    ),
                };
                let (x, y) = self.cursor;
                if let Err(e) = engine.data.app.on_mouse_wheel(dx, dy, x, y) {
                    log::error!("Handling the mouse wheel failed: {e:#}");
                }
            }
            // Touches, on Android, as the mouse (`touch.rs`). Windows sends mouse events for
            // touch screens itself.
            WindowEvent::Touch(Touch {
                id,
                phase,
                location,
                ..
            }) if cfg!(target_os = "android") => {
                let (x, y) = (location.x as f32, location.y as f32);
                for call in self.touch.touch(id, phase, x, y) {
                    let result = match call {
                        MouseCall::Moved(x, y) => {
                            self.cursor = (x, y);
                            engine.data.app.on_mouse_moved(x, y)
                        }
                        MouseCall::Button { pressed, x, y } => {
                            engine
                                .data
                                .app
                                .on_mouse_button(MouseButton::Left, pressed, x, y)
                        }
                        MouseCall::Left => engine.data.app.on_mouse_left(),
                    };
                    if let Err(e) = result {
                        log::error!("Handling a touch ({call:?}) failed: {e:#}");
                    }
                }
            }
            // F9 in debug builds: suspend and resume, as Android does (see `App::run`).
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::F9),
                        state: ElementState::Pressed,
                        repeat: false,
                        ..
                    },
                ..
            } if cfg!(debug_assertions) => unsafe {
                engine.suspend()?;
                engine.resume(window)?;
            },
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
                if let Err(e) = engine.data.app.on_key_pressed(key) {
                    log::error!("Handling key {key:?} failed: {e:#}");
                }
                engine.apply_window_title(window);
            }
            // Leave the loop; the engine is destroyed in `exiting`.
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => {}
        }
        Ok(())
    }
}
