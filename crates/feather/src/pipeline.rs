use std::collections::HashMap;

use anyhow::Result;
use vulkanalia::bytecode::Bytecode;
use vulkanalia::prelude::v1_0::*;

use super::appdata::AppData;
use super::dephobjects::get_depth_format;
use super::shader::Shader;
use super::shaderlayout::{ShaderLayout, ShaderStages};
use super::vertex::Vertex;

//================================================
// Pipeline
//================================================

pub unsafe fn create_render_pass(
    instance: &Instance,
    device: &Device,
    data: &mut AppData,
) -> Result<()> {
    // Attachments

    let color_attachment = vk::AttachmentDescription::builder()
        .format(data.swapchain.swapchain_format)
        .samples(data.msaa_samples)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

    let depth_stencil_attachment = vk::AttachmentDescription::builder()
        .format(get_depth_format(instance, data)?)
        .samples(data.msaa_samples)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::DONT_CARE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

    let color_resolve_attachment = vk::AttachmentDescription::builder()
        .format(data.swapchain.swapchain_format)
        .samples(vk::SampleCountFlags::_1)
        .load_op(vk::AttachmentLoadOp::DONT_CARE)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);

    // Subpasses

    let color_attachment_ref = vk::AttachmentReference::builder()
        .attachment(0)
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

    let depth_stencil_attachment_ref = vk::AttachmentReference::builder()
        .attachment(1)
        .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

    let color_resolve_attachment_ref = vk::AttachmentReference::builder()
        .attachment(2)
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

    let color_attachments = &[color_attachment_ref];
    let resolve_attachments = &[color_resolve_attachment_ref];
    let subpass = vk::SubpassDescription::builder()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(color_attachments)
        .depth_stencil_attachment(&depth_stencil_attachment_ref)
        .resolve_attachments(resolve_attachments);

    // Dependencies

    let dependency = vk::SubpassDependency::builder()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
        )
        .src_access_mask(vk::AccessFlags::empty())
        .dst_stage_mask(
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
        )
        .dst_access_mask(
            vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
        );

    // Create

    let attachments = &[
        color_attachment,
        depth_stencil_attachment,
        color_resolve_attachment,
    ];
    let subpasses = &[subpass];
    let dependencies = &[dependency];
    let info = vk::RenderPassCreateInfo::builder()
        .attachments(attachments)
        .subpasses(subpasses)
        .dependencies(dependencies);

    data.render_pass = device.create_render_pass(&info, None)?;

    Ok(())
}

/// What a pipeline renders into. Pipelines must be recreated when any of it changes (i.e.
/// with the swapchain).
pub struct PipelineTarget {
    pub render_pass: vk::RenderPass,
    pub msaa_samples: vk::SampleCountFlags,
    pub extent: vk::Extent2D,
}

/// Identifies a pipeline: a shader drawn either opaque or transparent (`Node::is_transparent`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PipelineKey {
    /// Handle of the shader in `Scene::shaders`.
    pub shader: usize,
    /// Alpha blending on and depth writes off.
    pub transparent: bool,
}

/// Descriptor sets for one swapchain image and one combination of textures.
struct CachedSets {
    sets: Vec<vk::DescriptorSet>,
    /// The uniform buffer the sets' uniform blocks point at.
    uniform_buffer: vk::Buffer,
}

/// Number of descriptor set groups (all sets of the shader) each descriptor pool holds.
const SET_GROUPS_PER_POOL: u32 = 16;

/// The Vulkan pipeline of one `Shader`, with layouts and descriptor sets created from the
/// shader's reflected `ShaderLayout`.
///
/// Every uniform block is a `UNIFORM_BUFFER_DYNAMIC` descriptor: all blocks of a frame live in
/// one buffer, and each draw passes the offsets of its own blocks. Samplers are bound per
/// draw, so descriptor sets are cached for each swapchain image and combination of textures.
pub struct ShaderPipeline {
    pub layout_info: ShaderLayout,
    /// One per descriptor set the shader uses (`ShaderLayout::set_count`).
    pub set_layouts: Vec<vk::DescriptorSetLayout>,
    pub layout: vk::PipelineLayout,
    pub pipeline: vk::Pipeline,
    /// Stages that read the push constants (empty if the shader has none).
    pub push_constant_stages: vk::ShaderStageFlags,
    /// Pools the descriptor sets come from; a new one is added when the last is full.
    descriptor_pools: Vec<vk::DescriptorPool>,
    /// Descriptor sets (set 0, 1, ...) by (swapchain image, image view of each sampler).
    descriptor_sets: HashMap<(usize, Vec<vk::ImageView>), CachedSets>,
}

impl ShaderPipeline {
    /// Creates the pipeline of `shader`. A `transparent` pipeline blends the shader's output
    /// over the framebuffer by its alpha (`src * a + dst * (1 - a)`) and tests depth without
    /// writing it, so transparent surfaces don't hide each other.
    pub unsafe fn create(
        device: &Device,
        target: &PipelineTarget,
        shader: &Shader,
        transparent: bool,
    ) -> Result<Self> {
        let shader_layout = shader.get_layout();
        let mut pipeline = Self {
            layout_info: shader_layout.clone(),
            set_layouts: Vec::new(),
            layout: vk::PipelineLayout::null(),
            pipeline: vk::Pipeline::null(),
            push_constant_stages: shader_layout
                .push_constants
                .as_ref()
                .map_or(vk::ShaderStageFlags::empty(), |p| stage_flags(p.stages)),
            descriptor_pools: Vec::new(),
            descriptor_sets: HashMap::new(),
        };
        // On error, destroy whatever was created so far.
        match pipeline.create_objects(device, target, shader, transparent) {
            Ok(()) => Ok(pipeline),
            Err(e) => {
                pipeline.destroy(device);
                Err(e)
            }
        }
    }

    unsafe fn create_objects(
        &mut self,
        device: &Device,
        target: &PipelineTarget,
        shader: &Shader,
        transparent: bool,
    ) -> Result<()> {
        let shader_layout = shader.get_layout();

        // Descriptor set layouts

        for set in 0..shader_layout.set_count() {
            let blocks = shader_layout
                .uniform_blocks
                .iter()
                .filter(|b| b.set == set)
                .map(|b| (b.binding, vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC, b.stages));
            let samplers = shader_layout
                .samplers
                .iter()
                .filter(|s| s.set == set)
                .map(|s| (s.binding, vk::DescriptorType::COMBINED_IMAGE_SAMPLER, s.stages));
            let bindings: Vec<_> = blocks
                .chain(samplers)
                .map(|(binding, descriptor_type, stages)| {
                    vk::DescriptorSetLayoutBinding::builder()
                        .binding(binding)
                        .descriptor_type(descriptor_type)
                        .descriptor_count(1)
                        .stage_flags(stage_flags(stages))
                        .build()
                })
                .collect();
            let info = vk::DescriptorSetLayoutCreateInfo::builder().bindings(&bindings);
            self.set_layouts
                .push(device.create_descriptor_set_layout(&info, None)?);
        }

        // Pipeline layout

        let push_constant_ranges: Vec<_> = shader_layout
            .push_constants
            .iter()
            .map(|p| {
                vk::PushConstantRange::builder()
                    .stage_flags(stage_flags(p.stages))
                    .offset(0)
                    .size(p.size)
                    .build()
            })
            .collect();
        let layout_info = vk::PipelineLayoutCreateInfo::builder()
            .set_layouts(&self.set_layouts)
            .push_constant_ranges(&push_constant_ranges);
        self.layout = device.create_pipeline_layout(&layout_info, None)?;

        self.pipeline = create_graphics_pipeline(device, target, shader, self.layout, transparent)?;
        Ok(())
    }

    /// The descriptor sets (set 0, 1, ...) for a draw on swapchain image `image`: uniform
    /// blocks point at `uniform_buffer` (draws add their dynamic offsets), sampler `i` at
    /// `views[i]`. Sets are created on first use and reused afterwards. The image's previous
    /// frame must be finished, and the sets must be fetched before recording the frame.
    pub unsafe fn descriptor_sets(
        &mut self,
        device: &Device,
        image: usize,
        views: &[vk::ImageView],
        uniform_buffer: vk::Buffer,
        sampler: vk::Sampler,
    ) -> Result<Vec<vk::DescriptorSet>> {
        if self.set_layouts.is_empty() {
            return Ok(Vec::new());
        }
        let key = (image, views.to_vec());
        if let Some(cached) = self.descriptor_sets.get_mut(&key) {
            if cached.uniform_buffer != uniform_buffer {
                // The frame's uniform buffer was replaced (it grew).
                write_descriptor_sets(device, &self.layout_info, &cached.sets, uniform_buffer, None);
                cached.uniform_buffer = uniform_buffer;
            }
            return Ok(cached.sets.clone());
        }

        let sets = self.allocate_sets(device)?;
        write_descriptor_sets(
            device,
            &self.layout_info,
            &sets,
            uniform_buffer,
            Some((views, sampler)),
        );
        self.descriptor_sets.insert(
            key,
            CachedSets {
                sets: sets.clone(),
                uniform_buffer,
            },
        );
        Ok(sets)
    }

    /// Allocates one descriptor set per set layout, adding a pool when the last one is full.
    unsafe fn allocate_sets(&mut self, device: &Device) -> Result<Vec<vk::DescriptorSet>> {
        if let Some(&pool) = self.descriptor_pools.last() {
            let info = vk::DescriptorSetAllocateInfo::builder()
                .descriptor_pool(pool)
                .set_layouts(&self.set_layouts);
            match device.allocate_descriptor_sets(&info) {
                Ok(sets) => return Ok(sets),
                Err(vk::ErrorCode::OUT_OF_POOL_MEMORY) | Err(vk::ErrorCode::FRAGMENTED_POOL) => {}
                Err(e) => return Err(e.into()),
            }
        }

        let pool_sizes: Vec<_> = [
            (
                vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
                self.layout_info.uniform_blocks.len() as u32,
            ),
            (
                vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                self.layout_info.samplers.len() as u32,
            ),
        ]
        .into_iter()
        .filter(|(_, count)| *count > 0)
        .map(|(type_, count)| {
            vk::DescriptorPoolSize::builder()
                .type_(type_)
                .descriptor_count(count * SET_GROUPS_PER_POOL)
                .build()
        })
        .collect();
        let info = vk::DescriptorPoolCreateInfo::builder()
            .pool_sizes(&pool_sizes)
            .max_sets(self.set_layouts.len() as u32 * SET_GROUPS_PER_POOL);
        let pool = device.create_descriptor_pool(&info, None)?;
        self.descriptor_pools.push(pool);

        let info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(pool)
            .set_layouts(&self.set_layouts);
        Ok(device.allocate_descriptor_sets(&info)?)
    }

    pub unsafe fn destroy(&self, device: &Device) {
        if self.pipeline != vk::Pipeline::null() {
            device.destroy_pipeline(self.pipeline, None);
        }
        self.descriptor_pools
            .iter()
            .for_each(|p| device.destroy_descriptor_pool(*p, None));
        if self.layout != vk::PipelineLayout::null() {
            device.destroy_pipeline_layout(self.layout, None);
        }
        self.set_layouts
            .iter()
            .for_each(|l| device.destroy_descriptor_set_layout(*l, None));
    }
}

/// Points the uniform blocks of `sets` at `uniform_buffer` and, when `textures` is given,
/// sampler `i` at `textures.0[i]` with sampler `textures.1`.
unsafe fn write_descriptor_sets(
    device: &Device,
    layout: &ShaderLayout,
    sets: &[vk::DescriptorSet],
    uniform_buffer: vk::Buffer,
    textures: Option<(&[vk::ImageView], vk::Sampler)>,
) {
    let buffer_infos: Vec<_> = layout
        .uniform_blocks
        .iter()
        .map(|b| {
            [vk::DescriptorBufferInfo::builder()
                .buffer(uniform_buffer)
                .offset(0)
                .range(b.size as u64)
                .build()]
        })
        .collect();
    let mut writes: Vec<_> = layout
        .uniform_blocks
        .iter()
        .zip(&buffer_infos)
        .map(|(b, info)| {
            vk::WriteDescriptorSet::builder()
                .dst_set(sets[b.set as usize])
                .dst_binding(b.binding)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC)
                .buffer_info(info)
                .build()
        })
        .collect();

    let image_infos: Vec<_> = textures
        .map(|(views, sampler)| {
            views
                .iter()
                .map(|view| {
                    [vk::DescriptorImageInfo::builder()
                        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                        .image_view(*view)
                        .sampler(sampler)
                        .build()]
                })
                .collect()
        })
        .unwrap_or_default();
    writes.extend(layout.samplers.iter().zip(&image_infos).map(|(s, info)| {
        vk::WriteDescriptorSet::builder()
            .dst_set(sets[s.set as usize])
            .dst_binding(s.binding)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .image_info(info)
            .build()
    }));

    device.update_descriptor_sets(&writes, &[] as &[vk::CopyDescriptorSet]);
}

fn stage_flags(stages: ShaderStages) -> vk::ShaderStageFlags {
    let mut flags = vk::ShaderStageFlags::empty();
    if stages.vertex {
        flags |= vk::ShaderStageFlags::VERTEX;
    }
    if stages.fragment {
        flags |= vk::ShaderStageFlags::FRAGMENT;
    }
    flags
}

unsafe fn create_graphics_pipeline(
    device: &Device,
    target: &PipelineTarget,
    shader: &Shader,
    layout: vk::PipelineLayout,
    transparent: bool,
) -> Result<vk::Pipeline> {
    // Stages

    let vert_shader_module = create_shader_module(device, shader.get_vertex())?;
    let frag_shader_module = match create_shader_module(device, shader.get_fragment()) {
        Ok(module) => module,
        Err(e) => {
            device.destroy_shader_module(vert_shader_module, None);
            return Err(e);
        }
    };

    let vert_stage = vk::PipelineShaderStageCreateInfo::builder()
        .stage(vk::ShaderStageFlags::VERTEX)
        .module(vert_shader_module)
        .name(b"main\0");

    let frag_stage = vk::PipelineShaderStageCreateInfo::builder()
        .stage(vk::ShaderStageFlags::FRAGMENT)
        .module(frag_shader_module)
        .name(b"main\0");

    // Vertex Input State

    let binding_descriptions = &[Vertex::binding_description()];
    let attribute_descriptions = Vertex::attribute_descriptions();
    let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::builder()
        .vertex_binding_descriptions(binding_descriptions)
        .vertex_attribute_descriptions(&attribute_descriptions);

    // Input Assembly State

    let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::builder()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
        .primitive_restart_enable(false);

    // Viewport State

    let viewport = vk::Viewport::builder()
        .x(0.0)
        .y(0.0)
        .width(target.extent.width as f32)
        .height(target.extent.height as f32)
        .min_depth(0.0)
        .max_depth(1.0);

    let scissor = vk::Rect2D::builder()
        .offset(vk::Offset2D { x: 0, y: 0 })
        .extent(target.extent);

    let viewports = &[viewport];
    let scissors = &[scissor];
    let viewport_state = vk::PipelineViewportStateCreateInfo::builder()
        .viewports(viewports)
        .scissors(scissors);

    // Rasterization State

    let rasterization_state = vk::PipelineRasterizationStateCreateInfo::builder()
        .depth_clamp_enable(false)
        .rasterizer_discard_enable(false)
        .polygon_mode(vk::PolygonMode::FILL)
        .line_width(1.0)
        .cull_mode(vk::CullModeFlags::BACK)
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
        .depth_bias_enable(false);

    // Multisample State

    let multisample_state = vk::PipelineMultisampleStateCreateInfo::builder()
        .sample_shading_enable(true)
        .min_sample_shading(0.2)
        .rasterization_samples(target.msaa_samples);

    // Depth Stencil State

    let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::builder()
        .depth_test_enable(true)
        .depth_write_enable(!transparent)
        .depth_compare_op(vk::CompareOp::LESS)
        .depth_bounds_test_enable(false)
        .stencil_test_enable(false);

    // Color Blend State

    let attachment = vk::PipelineColorBlendAttachmentState::builder()
        .color_write_mask(vk::ColorComponentFlags::all())
        .blend_enable(transparent)
        .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
        .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .color_blend_op(vk::BlendOp::ADD)
        .src_alpha_blend_factor(vk::BlendFactor::ONE)
        .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .alpha_blend_op(vk::BlendOp::ADD);

    let attachments = &[attachment];
    let color_blend_state = vk::PipelineColorBlendStateCreateInfo::builder()
        .logic_op_enable(false)
        .logic_op(vk::LogicOp::COPY)
        .attachments(attachments)
        .blend_constants([0.0, 0.0, 0.0, 0.0]);

    // Create

    let stages = &[vert_stage, frag_stage];
    let info = vk::GraphicsPipelineCreateInfo::builder()
        .stages(stages)
        .vertex_input_state(&vertex_input_state)
        .input_assembly_state(&input_assembly_state)
        .viewport_state(&viewport_state)
        .rasterization_state(&rasterization_state)
        .multisample_state(&multisample_state)
        .depth_stencil_state(&depth_stencil_state)
        .color_blend_state(&color_blend_state)
        .layout(layout)
        .render_pass(target.render_pass)
        .subpass(0);

    let result = device.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None);

    // Cleanup

    device.destroy_shader_module(vert_shader_module, None);
    device.destroy_shader_module(frag_shader_module, None);

    Ok(result?.0[0])
}

unsafe fn create_shader_module(device: &Device, bytecode: &[u8]) -> Result<vk::ShaderModule> {
    let bytecode = Bytecode::new(bytecode).unwrap();

    let info = vk::ShaderModuleCreateInfo::builder()
        .code_size(bytecode.code_size())
        .code(bytecode.code());

    Ok(device.create_shader_module(&info, None)?)
}
