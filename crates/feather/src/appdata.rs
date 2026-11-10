use std::collections::{HashMap, HashSet};

use vulkanalia::prelude::v1_0::*;

use super::buffers::FrameUniformBuffer;
use super::featherapp::FeatherApp;
use super::gpuimage::GpuImage;
use super::gpumeshbuffer::GpuMeshBuffer;
use super::pipeline::{PipelineKey, ShaderPipeline};
use super::uniformdata::UniformIssue;
use super::swapchain::Swapchain;

/// The Vulkan handles and associated properties used by our Vulkan app.
pub struct AppData {
    pub app: Box<dyn FeatherApp>,
    // Debug
    pub messenger: vk::DebugUtilsMessengerEXT,
    // Surface
    pub surface: vk::SurfaceKHR,
    // Physical Device / Logical Device
    pub physical_device: vk::PhysicalDevice,
    pub msaa_samples: vk::SampleCountFlags,
    pub graphics_queue: vk::Queue,
    pub present_queue: vk::Queue,
    // Swapchain
    pub swapchain: Swapchain,
    // Pipeline
    pub render_pass: vk::RenderPass,
    /// `minUniformBufferOffsetAlignment` of the device: uniform blocks of a frame start at
    /// multiples of it.
    pub min_uniform_alignment: u64,
    // Framebuffers
    pub framebuffers: Vec<vk::Framebuffer>,
    // Command Pool
    pub command_pool: vk::CommandPool,
    // Color
    pub color_image: vk::Image,
    pub color_image_memory: vk::DeviceMemory,
    pub color_image_view: vk::ImageView,
    // Depth
    pub depth_image: vk::Image,
    pub depth_image_memory: vk::DeviceMemory,
    pub depth_image_view: vk::ImageView,
    /// GPU state of each rendered scene, by `Scene::get_id`. It is keyed by the scene's
    /// handles, so each scene has its own; it is released when the scene is no longer
    /// rendered.
    pub scenes: HashMap<u64, SceneGpuState>,
    // Textures
    /// Sampler used for every texture.
    pub texture_sampler: vk::Sampler,
    /// 1x1 magenta image bound to samplers without a valid texture.
    pub fallback_texture: Option<GpuImage>,
    // Buffers
    /// Uniform data of each swapchain image's frame; created and grown when needed.
    pub frame_uniforms: Vec<Option<FrameUniformBuffer>>,
    // Command Buffers
    pub command_buffers: Vec<vk::CommandBuffer>,
    // Sync Objects
    pub image_available_semaphores: Vec<vk::Semaphore>,
    pub render_finished_semaphores: Vec<vk::Semaphore>,
    pub in_flight_fences: Vec<vk::Fence>,
    pub images_in_flight: Vec<vk::Fence>,
}

/// The GPU state of one rendered scene.
#[derive(Default)]
pub struct SceneGpuState {
    /// Pipelines of the scene's shaders, by shader handle and transparency. Created when first
    /// needed; destroyed with the swapchain.
    pub shader_pipelines: HashMap<PipelineKey, ShaderPipeline>,
    /// Pipelines that could not be created; their nodes are not drawn.
    pub failed_pipelines: HashSet<PipelineKey>,
    /// Uniform problems already logged, per shader handle, so each is logged once.
    pub reported_uniform_issues: HashSet<(usize, UniformIssue)>,
    /// Mesh buffers on the GPU, by mesh buffer handle.
    pub gpu_mesh_buffers: HashMap<usize, GpuMeshBuffer>,
    /// Atlases on the GPU, by atlas handle.
    pub gpu_atlases: HashMap<usize, GpuImage>,
    /// Atlases that could not be uploaded; their textures show the fallback texture.
    pub failed_atlases: HashSet<usize>,
}

impl SceneGpuState {
    /// Destroys the pipelines (they depend on the swapchain); they are recreated when needed.
    /// The GPU must not be using them.
    pub unsafe fn destroy_pipelines(&mut self, device: &Device) {
        self.shader_pipelines.drain().for_each(|(_, p)| p.destroy(device));
        self.failed_pipelines.clear();
    }

    /// Destroys everything. The GPU must not be using any of it.
    pub unsafe fn destroy(&mut self, device: &Device) {
        self.destroy_pipelines(device);
        self.gpu_atlases.drain().for_each(|(_, a)| a.destroy(device));
        self.gpu_mesh_buffers.drain().for_each(|(_, b)| b.destroy(device));
    }
}
