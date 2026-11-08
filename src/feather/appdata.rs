use std::collections::{HashMap, HashSet};

use vulkanalia::prelude::v1_0::*;

use super::buffers::FrameUniformBuffer;
use super::featherapp::FeatherApp;
use super::gpuimage::GpuImage;
use super::pipeline::ShaderPipeline;
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
    /// Pipelines of the rendered scene's shaders, by shader handle. Created when first
    /// needed; destroyed with the swapchain.
    pub shader_pipelines: HashMap<usize, ShaderPipeline>,
    /// Shaders whose pipeline could not be created; their nodes are not drawn.
    pub failed_shaders: HashSet<usize>,
    /// Uniform problems already logged, per shader handle, so each is logged once.
    pub reported_uniform_issues: HashSet<(usize, UniformIssue)>,
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
    // Textures
    /// Sampler used for every texture.
    pub texture_sampler: vk::Sampler,
    /// Atlases of the rendered scene on the GPU, by atlas handle.
    pub gpu_atlases: HashMap<usize, GpuImage>,
    /// Atlases that could not be uploaded; their textures show the fallback texture.
    pub failed_atlases: HashSet<usize>,
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
