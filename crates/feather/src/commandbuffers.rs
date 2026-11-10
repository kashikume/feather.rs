use anyhow::Result;
use vulkanalia::prelude::v1_0::*;

use super::appdata::AppData;
use super::pipeline::PipelineKey;
use super::shadowpass::{record_shadow_pass, ShadowPass};

//================================================
// Command Buffers
//================================================

/// One draw call with the Vulkan objects it uses.
pub struct GpuDraw {
    /// `Scene::get_id` of the draw's scene, whose GPU state has the pipeline.
    pub scene: u64,
    /// The pipeline (shader and transparency); it provides the descriptor sets.
    pub pipeline_key: PipelineKey,
    /// Whether to clear the depth buffer before this draw, so it and the draws after it are
    /// drawn over everything before (the first draw of every scene but the first).
    pub clear_depth: bool,
    /// Image view and sampler bound to each sampler of the shader.
    pub textures: Vec<(vk::ImageView, vk::Sampler)>,
    pub pipeline: vk::Pipeline,
    pub pipeline_layout: vk::PipelineLayout,
    /// The pipeline's descriptor sets for this draw (set 0, 1, ...); filled in once the
    /// frame's uniform buffer is ready.
    pub descriptor_sets: Vec<vk::DescriptorSet>,
    /// Offsets of the draw's uniform blocks in the frame's uniform buffer.
    pub dynamic_offsets: Vec<u32>,
    pub push_constants: Vec<u8>,
    pub push_constant_stages: vk::ShaderStageFlags,
    pub vertex_buffer: vk::Buffer,
    pub index_buffer: vk::Buffer,
    pub first_index: u32,
    pub index_count: u32,
    pub vertex_offset: i32,
}

/// Allocates one command buffer per framebuffer. They are recorded every frame by
/// `record_command_buffer`.
pub unsafe fn create_command_buffers(device: &Device, data: &mut AppData) -> Result<()> {
    if !data.command_buffers.is_empty() {
        device.free_command_buffers(data.command_pool, &data.command_buffers);
        data.command_buffers.clear();
    }

    let allocate_info = vk::CommandBufferAllocateInfo::builder()
        .command_pool(data.command_pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(data.framebuffers.len() as u32);

    data.command_buffers = device.allocate_command_buffers(&allocate_info)?;

    Ok(())
}

/// Records the command buffer of swapchain image `image_index`: the scenes' shadow passes,
/// then the main pass, which clears the image and issues `draws` (best sorted by pipeline).
/// The command buffer must not be in use by the GPU.
pub unsafe fn record_command_buffer(
    device: &Device,
    data: &AppData,
    image_index: usize,
    shadow_passes: &[ShadowPass],
    draws: &[GpuDraw],
) -> Result<()> {
    let command_buffer = data.command_buffers[image_index];

    device.reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::empty())?;

    let info = vk::CommandBufferBeginInfo::builder()
        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

    device.begin_command_buffer(command_buffer, &info)?;

    if let Some(renderer) = &data.shadow_renderer {
        for pass in shadow_passes {
            record_shadow_pass(device, command_buffer, renderer, pass);
        }
    }

    let render_area = vk::Rect2D::builder()
        .offset(vk::Offset2D::default())
        .extent(data.swapchain.swapchain_extent);

    let color_clear_value = vk::ClearValue {
        color: vk::ClearColorValue {
            float32: [0.0, 0.0, 0.0, 1.0],
        },
    };

    let depth_clear_value = vk::ClearValue {
        depth_stencil: vk::ClearDepthStencilValue {
            depth: 1.0,
            stencil: 0,
        },
    };

    let clear_values = &[color_clear_value, depth_clear_value];
    let info = vk::RenderPassBeginInfo::builder()
        .render_pass(data.render_pass)
        .framebuffer(data.framebuffers[image_index])
        .render_area(render_area)
        .clear_values(clear_values);

    device.cmd_begin_render_pass(command_buffer, &info, vk::SubpassContents::INLINE);

    let mut bound_pipeline = None;
    let mut bound_buffers = None;
    for draw in draws {
        if draw.clear_depth {
            let attachment = vk::ClearAttachment::builder()
                .aspect_mask(vk::ImageAspectFlags::DEPTH)
                .clear_value(depth_clear_value)
                .build();
            let rect = vk::ClearRect::builder()
                .rect(*render_area)
                .base_array_layer(0)
                .layer_count(1)
                .build();
            device.cmd_clear_attachments(command_buffer, &[attachment], &[rect]);
        }

        if bound_pipeline != Some(draw.pipeline) {
            device.cmd_bind_pipeline(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                draw.pipeline,
            );
            bound_pipeline = Some(draw.pipeline);
        }

        if !draw.descriptor_sets.is_empty() {
            device.cmd_bind_descriptor_sets(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                draw.pipeline_layout,
                0,
                &draw.descriptor_sets,
                &draw.dynamic_offsets,
            );
        }

        if !draw.push_constants.is_empty() {
            device.cmd_push_constants(
                command_buffer,
                draw.pipeline_layout,
                draw.push_constant_stages,
                0,
                &draw.push_constants,
            );
        }

        // Meshes sharing a mesh buffer don't need the buffers bound again.
        if bound_buffers != Some((draw.vertex_buffer, draw.index_buffer)) {
            device.cmd_bind_vertex_buffers(command_buffer, 0, &[draw.vertex_buffer], &[0]);
            device.cmd_bind_index_buffer(
                command_buffer,
                draw.index_buffer,
                0,
                vk::IndexType::UINT32,
            );
            bound_buffers = Some((draw.vertex_buffer, draw.index_buffer));
        }

        device.cmd_draw_indexed(
            command_buffer,
            draw.index_count,
            1,
            draw.first_index,
            draw.vertex_offset,
            0,
        );
    }

    device.cmd_end_render_pass(command_buffer);
    device.end_command_buffer(command_buffer)?;

    Ok(())
}
