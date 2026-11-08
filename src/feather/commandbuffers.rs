use std::mem::size_of;
use std::slice;

use anyhow::Result;
use vulkanalia::prelude::v1_0::*;

use super::appdata::AppData;
use super::math::Mat4;
use super::pipeline::PushConstantSlot;

//================================================
// Command Buffers
//================================================

/// One draw call with the Vulkan objects it uses.
pub struct GpuDraw {
    pub pipeline: vk::Pipeline,
    pub pipeline_layout: vk::PipelineLayout,
    /// Where to push the model matrix; `None` if the shader doesn't take it as a push constant.
    pub model_push_constant: Option<PushConstantSlot>,
    pub vertex_buffer: vk::Buffer,
    pub index_buffer: vk::Buffer,
    pub model: Mat4,
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

/// Records the command buffer of swapchain image `image_index`: clears the image and issues
/// `draws` (best sorted by pipeline). The command buffer must not be in use by the GPU.
pub unsafe fn record_command_buffer(
    device: &Device,
    data: &AppData,
    image_index: usize,
    draws: &[GpuDraw],
) -> Result<()> {
    let command_buffer = data.command_buffers[image_index];

    device.reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::empty())?;

    let info = vk::CommandBufferBeginInfo::builder()
        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

    device.begin_command_buffer(command_buffer, &info)?;

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
        if bound_pipeline != Some(draw.pipeline) {
            device.cmd_bind_pipeline(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                draw.pipeline,
            );
            device.cmd_bind_descriptor_sets(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                draw.pipeline_layout,
                0,
                &[data.descriptor_sets[image_index]],
                &[],
            );
            bound_pipeline = Some(draw.pipeline);
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

        if let Some(slot) = draw.model_push_constant {
            let model_bytes = slice::from_raw_parts(
                (&draw.model as *const Mat4).cast::<u8>(),
                size_of::<Mat4>(),
            );
            device.cmd_push_constants(
                command_buffer,
                draw.pipeline_layout,
                slot.stages,
                slot.offset,
                model_bytes,
            );
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
