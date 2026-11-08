use std::collections::{HashMap, HashSet};
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
use super::pipeline::{create_render_pass, PipelineTarget, ShaderPipeline};
use super::swapchain::Swapchain;
use super::syncobjects::create_sync_objects;
use super::gpuimage::{create_texture_sampler, GpuImage, UploadContext};
use super::gpumeshbuffer::GpuMeshBuffer;
use super::uniform::UniformValue;
use super::uniformdata::{pack_draw, resolve_samplers, UniformArena, UniformValues};

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
            shader_pipelines: HashMap::new(),
            failed_shaders: HashSet::new(),
            reported_uniform_issues: HashSet::new(),
            min_uniform_alignment: 1,
            framebuffers: Vec::new(),
            command_pool: vk::CommandPool::default(),
            color_image: vk::Image::default(),
            color_image_memory: vk::DeviceMemory::default(),
            color_image_view: vk::ImageView::default(),
            depth_image: vk::Image::default(),
            depth_image_memory: vk::DeviceMemory::default(),
            depth_image_view: vk::ImageView::default(),
            rendered_scene: None,
            gpu_mesh_buffers: HashMap::new(),
            texture_sampler: vk::Sampler::default(),
            gpu_atlases: HashMap::new(),
            failed_atlases: HashSet::new(),
            fallback_texture: None,
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

    /// Gets the scenes ready for drawing: places new meshes in mesh buffers and updates node
    /// uniforms and transforms.
    fn prepare_scenes(&mut self) -> Result<()> {
        for i in 0..self.data.app.get_num_scenes_to_render() {
            let scene = self.data.app.get_scene_to_render(i);
            scene.build_missing_mesh_buffers()?;
            scene.update_nodes();
        }
        Ok(())
    }

    /// Releases all GPU state of the rendered scene: mesh buffers, atlases and pipelines (with
    /// their descriptor sets). Waits for the GPU first.
    unsafe fn release_scene_gpu_state(&mut self) -> Result<()> {
        self.device.device_wait_idle()?;
        let device = &self.device;
        self.data.shader_pipelines.drain().for_each(|(_, p)| p.destroy(device));
        self.data.gpu_atlases.drain().for_each(|(_, a)| a.destroy(device));
        self.data.gpu_mesh_buffers.drain().for_each(|(_, b)| b.destroy(device));
        self.data.failed_shaders.clear();
        self.data.failed_atlases.clear();
        self.data.reported_uniform_issues.clear();
        Ok(())
    }

    /// Collects the draw calls of the rendered scene, sorted by shader, and packs their
    /// uniforms into the returned arena. Creates the pipelines of
    /// shaders drawn for the first time.
    ///
    /// Only the first scene is drawn for now.
    unsafe fn collect_draws(&mut self) -> Result<(Vec<GpuDraw>, UniformArena)> {
        let mut arena = UniformArena::new(self.data.min_uniform_alignment as usize);
        let num_scenes = self.data.app.get_num_scenes_to_render();
        if num_scenes == 0 {
            return Ok((Vec::new(), arena));
        }
        if num_scenes > 1 {
            static WARN: std::sync::Once = std::sync::Once::new();
            WARN.call_once(|| log::warn!("Only the first of {num_scenes} scenes is rendered"));
        }

        let extent = self.data.swapchain.swapchain_extent;
        let camera = self.data.app.get_camera_to_render_scene(0);
        camera.set_screen_dimention(extent.width, extent.height);
        let (view, proj) = (camera.get_view(), camera.get_projection());

        let scene = self.data.app.get_scene_to_render(0);

        // Pipelines of shaders that no longer exist. They may still be used by frames in
        // flight, so wait for the GPU before destroying them.
        let removed: Vec<usize> = self
            .data
            .shader_pipelines
            .keys()
            .copied()
            .filter(|&h| scene.get_shader(h).is_none())
            .collect();
        if !removed.is_empty() {
            self.device.device_wait_idle()?;
            for handle in removed {
                self.data.shader_pipelines.remove(&handle).unwrap().destroy(&self.device);
            }
        }
        self.data
            .failed_shaders
            .retain(|&h| scene.get_shader(h).is_some());

        let mut items = scene.draw_items();
        items.sort_by_key(|item| item.shader);

        let target = PipelineTarget {
            render_pass: self.data.render_pass,
            msaa_samples: self.data.msaa_samples,
            extent,
        };
        let mut draws = Vec::with_capacity(items.len());
        let mut issues = Vec::new();
        for item in items {
            if self.data.failed_shaders.contains(&item.shader) {
                continue;
            }
            let shader = scene.get_shader(item.shader).unwrap();
            if !self.data.shader_pipelines.contains_key(&item.shader) {
                match ShaderPipeline::create(&self.device, &target, shader) {
                    Ok(pipeline) => {
                        self.data.shader_pipelines.insert(item.shader, pipeline);
                    }
                    Err(e) => {
                        log::error!(
                            "Cannot draw with shader {}: {e:#}",
                            shader.get_name().unwrap_or_else(|| item.shader.to_string())
                        );
                        self.data.failed_shaders.insert(item.shader);
                        continue;
                    }
                }
            }
            let pipeline = &self.data.shader_pipelines[&item.shader];
            let Some(buffer) = self.data.gpu_mesh_buffers.get(&item.mesh_buffer) else {
                continue;
            };
            let (vertex_buffer, index_buffer) = (buffer.vertex_buffer, buffer.index_buffer);
            let Some(node) = scene.get_node(item.node) else {
                continue;
            };

            // Uniforms: the node's dynamic uniforms, plus the matrices the engine provides
            // under the names the shader gave them.
            let mut engine: Vec<(&str, UniformValue)> = [
                (shader.get_uniform_model_name(), item.model),
                (shader.get_uniform_view_name(), view),
                (shader.get_uniform_proj_name(), proj),
            ]
            .into_iter()
            .filter_map(|(name, matrix)| Some((name?, UniformValue::Mat4(matrix))))
            .collect();
            issues.clear();

            // Textures: the atlas of each sampler's texture, and its UV rectangle under
            // `<sampler>Rect`. Samplers without a valid texture get the fallback texture.
            let samplers = resolve_samplers(
                shader.get_layout(),
                &UniformValues {
                    engine: &engine,
                    node: node.dynamic_uniforms(),
                },
                &scene.textures,
                &mut issues,
            );
            let fallback_view = self.data.fallback_texture.as_ref().unwrap().view;
            let texture_views = samplers
                .atlases
                .iter()
                .map(|atlas| {
                    atlas
                        .and_then(|a| self.data.gpu_atlases.get(&a))
                        .map_or(fallback_view, |image| image.view)
                })
                .collect();
            engine.extend(samplers.uv_rects.iter().map(|(name, v)| (name.as_str(), *v)));

            let values = UniformValues {
                engine: &engine,
                node: node.dynamic_uniforms(),
            };
            let packed = pack_draw(shader.get_layout(), &values, &mut arena, &mut issues);
            for issue in issues.drain(..) {
                if self
                    .data
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
                shader: item.shader,
                texture_views,
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
        Ok((draws, arena))
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
            let pipeline = self.data.shader_pipelines.get_mut(&draw.shader).unwrap();
            draw.descriptor_sets = pipeline.descriptor_sets(
                &self.device,
                image_index,
                &draw.texture_views,
                buffer.buffer,
                self.data.texture_sampler,
            )?;
        }
        Ok(())
    }

    /// Brings the GPU state in line with the rendered scene: when the app renders a different
    /// scene, releases everything of the previous one; uploads new mesh buffers and atlases,
    /// and releases removed ones.
    unsafe fn sync_gpu_state(&mut self) -> Result<()> {
        if self.data.app.get_num_scenes_to_render() == 0 {
            return Ok(());
        }
        let scene_id = self.data.app.get_scene_to_render(0).get_id();
        if self.data.rendered_scene != Some(scene_id) {
            if self.data.rendered_scene.is_some() {
                self.release_scene_gpu_state()?;
            }
            self.data.rendered_scene = Some(scene_id);
        }
        let scene = self.data.app.get_scene_to_render(0);

        let upload = UploadContext {
            instance: &self.instance,
            device: &self.device,
            physical_device: self.data.physical_device,
            command_pool: self.data.command_pool,
            queue: self.data.graphics_queue,
        };

        // Mesh buffers. Their contents never change once created.
        let removed: Vec<usize> = self
            .data
            .gpu_mesh_buffers
            .keys()
            .copied()
            .filter(|&h| scene.buffers.get(h).is_none())
            .collect();
        if !removed.is_empty() {
            self.device.device_wait_idle()?;
            for handle in removed {
                self.data.gpu_mesh_buffers.remove(&handle).unwrap().destroy(&self.device);
            }
        }
        for buffer in scene.buffers.iter() {
            let handle = buffer.get_handle();
            if !self.data.gpu_mesh_buffers.contains_key(&handle) {
                let gpu_buffer = GpuMeshBuffer::upload(&upload, buffer, &scene.meshes)?;
                self.data.gpu_mesh_buffers.insert(handle, gpu_buffer);
            }
        }

        // Atlases.

        let removed: Vec<usize> = self
            .data
            .gpu_atlases
            .keys()
            .copied()
            .filter(|&h| scene.get_atlas(h).is_none())
            .collect();
        if !removed.is_empty() {
            // Frames in flight and cached descriptor sets may use the images. Pipelines,
            // with their descriptor sets, are recreated when next needed.
            self.device.device_wait_idle()?;
            self.data
                .shader_pipelines
                .drain()
                .for_each(|(_, p)| p.destroy(&self.device));
            for handle in removed {
                self.data.gpu_atlases.remove(&handle).unwrap().destroy(&self.device);
            }
        }
        self.data
            .failed_atlases
            .retain(|&h| scene.get_atlas(h).is_some());

        for atlas in scene.atlases.iter() {
            let handle = atlas.get_handle();
            if self.data.gpu_atlases.contains_key(&handle)
                || self.data.failed_atlases.contains(&handle)
            {
                continue;
            }
            match GpuImage::upload_rgba(
                &upload,
                atlas.get_width(),
                atlas.get_height(),
                atlas.get_pixels(),
            ) {
                Ok(image) => {
                    self.data.gpu_atlases.insert(handle, image);
                }
                Err(e) => {
                    log::error!(
                        "Cannot upload atlas {}: {e:#}",
                        atlas.get_name().unwrap_or_else(|| handle.to_string())
                    );
                    self.data.failed_atlases.insert(handle);
                }
            }
        }
        Ok(())
    }

    /// Renders a frame for our Vulkan app.
    pub unsafe fn render(&mut self, window: &Window) -> Result<()> {
        let time = self.start.elapsed().as_secs_f32();
        self.data.app.on_update(time)?;
        self.prepare_scenes()?;
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

        let (mut draws, arena) = self.collect_draws()?;
        self.upload_uniforms(image_index, &arena, &mut draws)?;
        record_command_buffer(&self.device, &self.data, image_index, &draws)?;

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
        self.data.gpu_mesh_buffers.drain().for_each(|(_, b)| b.destroy(&self.device));
        self.data.gpu_atlases.drain().for_each(|(_, a)| a.destroy(&self.device));
        if let Some(fallback) = self.data.fallback_texture.take() {
            fallback.destroy(&self.device);
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
        self.data.shader_pipelines.drain().for_each(|(_, p)| p.destroy(&self.device));
        self.data.failed_shaders.clear();
        self.device.destroy_render_pass(self.data.render_pass, None);
        self.data.swapchain.destroy(&self.device);
    }

    pub fn run(&mut self, window: &Window, event_loop: EventLoop<()>) -> Result<()> {
        let mut minimized = false;
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
