use anyhow::{bail, Result};
use vulkanalia::bytecode::Bytecode;
use vulkanalia::prelude::v1_0::*;

use super::appdata::AppData;
use super::dephobjects::get_depth_format;
use super::shader::Shader;
use super::shaderlayout::{ShaderStages, UniformLocation};
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

pub unsafe fn create_descriptor_set_layout(device: &Device, data: &mut AppData) -> Result<()> {
    let ubo_binding = vk::DescriptorSetLayoutBinding::builder()
        .binding(0)
        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
        .descriptor_count(1)
        .stage_flags(vk::ShaderStageFlags::VERTEX);

    let sampler_binding = vk::DescriptorSetLayoutBinding::builder()
        .binding(1)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .descriptor_count(1)
        .stage_flags(vk::ShaderStageFlags::FRAGMENT);

    let bindings = &[ubo_binding, sampler_binding];
    let info = vk::DescriptorSetLayoutCreateInfo::builder().bindings(bindings);

    data.descriptor_set_layout = device.create_descriptor_set_layout(&info, None)?;

    Ok(())
}

/// What a pipeline renders into. Pipelines must be recreated when any of it changes (i.e.
/// with the swapchain).
pub struct PipelineTarget {
    pub render_pass: vk::RenderPass,
    pub msaa_samples: vk::SampleCountFlags,
    pub extent: vk::Extent2D,
}

/// The Vulkan pipeline of one `Shader`, with layouts created from the shader's reflected
/// `ShaderLayout`.
pub struct ShaderPipeline {
    /// One per descriptor set the shader uses (`ShaderLayout::set_count`).
    pub set_layouts: Vec<vk::DescriptorSetLayout>,
    pub layout: vk::PipelineLayout,
    pub pipeline: vk::Pipeline,
    /// Where the node's model matrix goes, if the shader's model uniform is a push constant.
    pub model_push_constant: Option<PushConstantSlot>,
}

/// A push constant the renderer writes.
#[derive(Clone, Copy)]
pub struct PushConstantSlot {
    pub offset: u32,
    pub stages: vk::ShaderStageFlags,
}

impl ShaderPipeline {
    pub unsafe fn create(device: &Device, target: &PipelineTarget, shader: &Shader) -> Result<Self> {
        let shader_layout = shader.get_layout();

        // Descriptor set layouts

        let mut set_layouts = Vec::new();
        for set in 0..shader_layout.set_count() {
            let blocks = shader_layout
                .uniform_blocks
                .iter()
                .filter(|b| b.set == set)
                .map(|b| (b.binding, vk::DescriptorType::UNIFORM_BUFFER, b.stages));
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
            set_layouts.push(device.create_descriptor_set_layout(&info, None)?);
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
            .set_layouts(&set_layouts)
            .push_constant_ranges(&push_constant_ranges);
        let layout = device.create_pipeline_layout(&layout_info, None)?;

        let model_push_constant = shader.get_uniform_model_name().and_then(|name| {
            match shader_layout.find(name) {
                Some(UniformLocation::PushConstant(member)) => Some(PushConstantSlot {
                    offset: member.offset,
                    stages: stage_flags(shader_layout.push_constants.as_ref()?.stages),
                }),
                _ => None,
            }
        });

        let pipeline = create_graphics_pipeline(device, target, shader, layout);
        match pipeline {
            Ok(pipeline) => Ok(Self {
                set_layouts,
                layout,
                pipeline,
                model_push_constant,
            }),
            Err(e) => {
                device.destroy_pipeline_layout(layout, None);
                set_layouts
                    .iter()
                    .for_each(|l| device.destroy_descriptor_set_layout(*l, None));
                Err(e)
            }
        }
    }

    pub unsafe fn destroy(&self, device: &Device) {
        device.destroy_pipeline(self.pipeline, None);
        device.destroy_pipeline_layout(self.layout, None);
        self.set_layouts
            .iter()
            .for_each(|l| device.destroy_descriptor_set_layout(*l, None));
    }
}

/// Checks that the renderer can feed `shader`. Until uniform data and textures come from the
/// scene (steps 3 and 4 in `doc/todo/node-uniforms-rendering.md`), every shader shares one
/// fixed descriptor set (`create_descriptor_set_layout`): binding 0 = uniform buffer with the
/// view and projection matrices at offsets 0 and 64 (vertex stage), binding 1 = the texture
/// (fragment stage).
pub fn check_renderer_support(shader: &Shader) -> Result<()> {
    let layout = shader.get_layout();
    let blocks: Vec<_> = layout
        .uniform_blocks
        .iter()
        .map(|b| (b.set, b.binding, b.stages))
        .collect();
    let samplers: Vec<_> = layout
        .samplers
        .iter()
        .map(|s| (s.set, s.binding, s.stages))
        .collect();
    if blocks != [(0, 0, ShaderStages::VERTEX)] || samplers != [(0, 1, ShaderStages::FRAGMENT)] {
        bail!(
            "the renderer only supports one uniform block at set 0 binding 0 (vertex stage) \
             and one sampler2D at set 0 binding 1 (fragment stage)"
        );
    }
    for (what, name, offset) in [
        ("view", shader.get_uniform_view_name(), 0),
        ("projection", shader.get_uniform_proj_name(), 64),
    ] {
        let Some(name) = name else {
            bail!("the shader has no {what} matrix uniform name");
        };
        match layout.find(name) {
            Some(UniformLocation::Block { member, .. }) if member.offset == offset => {}
            _ => bail!("the {what} matrix `{name}` must be in the uniform block at offset {offset}"),
        }
    }
    Ok(())
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
        .depth_write_enable(true)
        .depth_compare_op(vk::CompareOp::LESS)
        .depth_bounds_test_enable(false)
        .stencil_test_enable(false);

    // Color Blend State

    let attachment = vk::PipelineColorBlendAttachmentState::builder()
        .color_write_mask(vk::ColorComponentFlags::all())
        .blend_enable(false);

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
