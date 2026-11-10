//! The shadow pass on the GPU: drawing shadow casters from a scene's shadow light into its
//! shadow map, before the frame's main render pass. See `shadow` for how shaders use it.

use anyhow::{anyhow, Result};
use vulkanalia::bytecode::Bytecode;
use vulkanalia::prelude::v1_0::*;

use super::images::{create_image, create_image_view};
use super::math::Mat4;
use super::vertex::Vertex;

/// The shadow pass's vertex shader (`shaders/shadow.vert`).
const SHADOW_VERT: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/engine_shaders/shadow.vert.spv"));

/// Size of the shadow pass's push constants: the light's matrix and the caster's model matrix.
const PUSH_CONSTANTS_SIZE: u32 = 2 * 64;

/// What every shadow pass uses: the depth format, the render pass, the depth-only pipeline and
/// the sampler shaders read shadow maps with. Independent of the swapchain.
pub struct ShadowRenderer {
    pub format: vk::Format,
    pub render_pass: vk::RenderPass,
    pub layout: vk::PipelineLayout,
    pub pipeline: vk::Pipeline,
    /// Nearest filtering, and a white border: places outside the map are lit.
    pub sampler: vk::Sampler,
}

impl ShadowRenderer {
    pub unsafe fn create(
        instance: &Instance,
        device: &Device,
        physical_device: vk::PhysicalDevice,
    ) -> Result<Self> {
        let format = shadow_format(instance, physical_device)?;
        let mut renderer = Self {
            format,
            render_pass: vk::RenderPass::null(),
            layout: vk::PipelineLayout::null(),
            pipeline: vk::Pipeline::null(),
            sampler: vk::Sampler::null(),
        };
        if let Err(e) = renderer.create_objects(device) {
            renderer.destroy(device);
            return Err(e);
        }
        Ok(renderer)
    }

    unsafe fn create_objects(&mut self, device: &Device) -> Result<()> {
        self.render_pass = create_shadow_render_pass(device, self.format)?;

        let ranges = &[vk::PushConstantRange::builder()
            .stage_flags(vk::ShaderStageFlags::VERTEX)
            .offset(0)
            .size(PUSH_CONSTANTS_SIZE)
            .build()];
        let info = vk::PipelineLayoutCreateInfo::builder().push_constant_ranges(ranges);
        self.layout = device.create_pipeline_layout(&info, None)?;
        self.pipeline = create_shadow_pipeline(device, self.render_pass, self.layout)?;

        let info = vk::SamplerCreateInfo::builder()
            .mag_filter(vk::Filter::NEAREST)
            .min_filter(vk::Filter::NEAREST)
            .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_BORDER)
            .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_BORDER)
            .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_BORDER)
            .border_color(vk::BorderColor::FLOAT_OPAQUE_WHITE)
            .compare_enable(false)
            .min_lod(0.0)
            .max_lod(0.0);
        self.sampler = device.create_sampler(&info, None)?;
        Ok(())
    }

    pub unsafe fn destroy(&self, device: &Device) {
        if self.sampler != vk::Sampler::null() {
            device.destroy_sampler(self.sampler, None);
        }
        if self.pipeline != vk::Pipeline::null() {
            device.destroy_pipeline(self.pipeline, None);
        }
        if self.layout != vk::PipelineLayout::null() {
            device.destroy_pipeline_layout(self.layout, None);
        }
        if self.render_pass != vk::RenderPass::null() {
            device.destroy_render_pass(self.render_pass, None);
        }
    }
}

/// A depth format that can be rendered to and sampled.
unsafe fn shadow_format(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
) -> Result<vk::Format> {
    let features =
        vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT | vk::FormatFeatureFlags::SAMPLED_IMAGE;
    [vk::Format::D32_SFLOAT, vk::Format::D16_UNORM]
        .into_iter()
        .find(|&format| {
            instance
                .get_physical_device_format_properties(physical_device, format)
                .optimal_tiling_features
                .contains(features)
        })
        .ok_or_else(|| anyhow!("no depth format can be used for shadow maps"))
}

/// One depth attachment, cleared to 1 and left ready to be sampled by the main pass.
unsafe fn create_shadow_render_pass(device: &Device, format: vk::Format) -> Result<vk::RenderPass> {
    let attachment = vk::AttachmentDescription::builder()
        .format(format)
        .samples(vk::SampleCountFlags::_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
    let depth_ref = vk::AttachmentReference::builder()
        .attachment(0)
        .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
    let subpass = vk::SubpassDescription::builder()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .depth_stencil_attachment(&depth_ref);

    let dependencies = &[
        // Earlier frames' fragment shaders may still read the map: wait for them before writing.
        vk::SubpassDependency::builder()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
            .src_access_mask(vk::AccessFlags::SHADER_READ)
            .dst_stage_mask(
                vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                    | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
            )
            .dst_access_mask(vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
            .build(),
        // The main pass's fragment shaders read the map once it is written.
        vk::SubpassDependency::builder()
            .src_subpass(0)
            .dst_subpass(vk::SUBPASS_EXTERNAL)
            .src_stage_mask(vk::PipelineStageFlags::LATE_FRAGMENT_TESTS)
            .src_access_mask(vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
            .dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
            .dst_access_mask(vk::AccessFlags::SHADER_READ)
            .build(),
    ];

    let attachments = &[attachment];
    let subpasses = &[subpass];
    let info = vk::RenderPassCreateInfo::builder()
        .attachments(attachments)
        .subpasses(subpasses)
        .dependencies(dependencies);
    Ok(device.create_render_pass(&info, None)?)
}

/// Depth only: positions from the vertex buffer, no fragment shader, both faces drawn (so
/// open meshes like planes cast shadows too), with a depth bias against shadow acne.
unsafe fn create_shadow_pipeline(
    device: &Device,
    render_pass: vk::RenderPass,
    layout: vk::PipelineLayout,
) -> Result<vk::Pipeline> {
    let bytecode = Bytecode::new(SHADOW_VERT)?;
    let info = vk::ShaderModuleCreateInfo::builder()
        .code_size(bytecode.code_size())
        .code(bytecode.code());
    let module = device.create_shader_module(&info, None)?;

    let stages = &[vk::PipelineShaderStageCreateInfo::builder()
        .stage(vk::ShaderStageFlags::VERTEX)
        .module(module)
        .name(b"main\0")
        .build()];
    let bindings = &[Vertex::binding_description()];
    let attributes = &[Vertex::attribute_descriptions()[0]];
    let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::builder()
        .vertex_binding_descriptions(bindings)
        .vertex_attribute_descriptions(attributes);
    let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::builder()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
    // The map's size varies: viewport and scissor are set when recording.
    let viewport_state = vk::PipelineViewportStateCreateInfo::builder()
        .viewport_count(1)
        .scissor_count(1);
    let rasterization_state = vk::PipelineRasterizationStateCreateInfo::builder()
        .polygon_mode(vk::PolygonMode::FILL)
        .line_width(1.0)
        .cull_mode(vk::CullModeFlags::NONE)
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
        .depth_bias_enable(true)
        .depth_bias_constant_factor(1.5)
        .depth_bias_slope_factor(2.0);
    let multisample_state = vk::PipelineMultisampleStateCreateInfo::builder()
        .rasterization_samples(vk::SampleCountFlags::_1);
    let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::builder()
        .depth_test_enable(true)
        .depth_write_enable(true)
        .depth_compare_op(vk::CompareOp::LESS);
    let dynamic_states = &[vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
    let dynamic_state =
        vk::PipelineDynamicStateCreateInfo::builder().dynamic_states(dynamic_states);

    let info = vk::GraphicsPipelineCreateInfo::builder()
        .stages(stages)
        .vertex_input_state(&vertex_input_state)
        .input_assembly_state(&input_assembly_state)
        .viewport_state(&viewport_state)
        .rasterization_state(&rasterization_state)
        .multisample_state(&multisample_state)
        .depth_stencil_state(&depth_stencil_state)
        .dynamic_state(&dynamic_state)
        .layout(layout)
        .render_pass(render_pass)
        .subpass(0);
    let result = device.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None);
    device.destroy_shader_module(module, None);
    Ok(result?.0[0])
}

/// A scene's shadow map: a square depth image with its framebuffer.
pub struct ShadowMap {
    pub size: u32,
    image: vk::Image,
    memory: vk::DeviceMemory,
    pub view: vk::ImageView,
    pub framebuffer: vk::Framebuffer,
}

impl ShadowMap {
    pub unsafe fn create(
        instance: &Instance,
        device: &Device,
        physical_device: vk::PhysicalDevice,
        renderer: &ShadowRenderer,
        size: u32,
    ) -> Result<Self> {
        let (image, memory) = create_image(
            instance,
            device,
            physical_device,
            size,
            size,
            1,
            vk::SampleCountFlags::_1,
            renderer.format,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;
        let mut map = Self {
            size,
            image,
            memory,
            view: vk::ImageView::null(),
            framebuffer: vk::Framebuffer::null(),
        };
        let result = (|| -> Result<()> {
            map.view = create_image_view(
                device,
                image,
                renderer.format,
                vk::ImageAspectFlags::DEPTH,
                1,
            )?;
            let attachments = &[map.view];
            let info = vk::FramebufferCreateInfo::builder()
                .render_pass(renderer.render_pass)
                .attachments(attachments)
                .width(size)
                .height(size)
                .layers(1);
            map.framebuffer = device.create_framebuffer(&info, None)?;
            Ok(())
        })();
        if let Err(e) = result {
            map.destroy(device);
            return Err(e);
        }
        Ok(map)
    }

    /// The GPU must not be using it.
    pub unsafe fn destroy(&self, device: &Device) {
        if self.framebuffer != vk::Framebuffer::null() {
            device.destroy_framebuffer(self.framebuffer, None);
        }
        if self.view != vk::ImageView::null() {
            device.destroy_image_view(self.view, None);
        }
        device.destroy_image(self.image, None);
        device.free_memory(self.memory, None);
    }
}

/// One shadow caster to draw.
pub struct ShadowDraw {
    pub model: Mat4,
    pub vertex_buffer: vk::Buffer,
    pub index_buffer: vk::Buffer,
    pub first_index: u32,
    pub index_count: u32,
    pub vertex_offset: i32,
}

/// The shadow pass of one scene.
pub struct ShadowPass {
    pub framebuffer: vk::Framebuffer,
    pub size: u32,
    pub light: Mat4,
    pub draws: Vec<ShadowDraw>,
}

/// Records `pass` into `command_buffer`, outside any render pass.
pub unsafe fn record_shadow_pass(
    device: &Device,
    command_buffer: vk::CommandBuffer,
    renderer: &ShadowRenderer,
    pass: &ShadowPass,
) {
    let extent = vk::Extent2D {
        width: pass.size,
        height: pass.size,
    };
    let render_area = vk::Rect2D::builder()
        .offset(vk::Offset2D::default())
        .extent(extent)
        .build();
    let clear_values = &[vk::ClearValue {
        depth_stencil: vk::ClearDepthStencilValue {
            depth: 1.0,
            stencil: 0,
        },
    }];
    let info = vk::RenderPassBeginInfo::builder()
        .render_pass(renderer.render_pass)
        .framebuffer(pass.framebuffer)
        .render_area(render_area)
        .clear_values(clear_values);
    device.cmd_begin_render_pass(command_buffer, &info, vk::SubpassContents::INLINE);
    device.cmd_bind_pipeline(
        command_buffer,
        vk::PipelineBindPoint::GRAPHICS,
        renderer.pipeline,
    );
    let viewport = vk::Viewport::builder()
        .width(pass.size as f32)
        .height(pass.size as f32)
        .min_depth(0.0)
        .max_depth(1.0)
        .build();
    device.cmd_set_viewport(command_buffer, 0, &[viewport]);
    device.cmd_set_scissor(command_buffer, 0, &[render_area]);

    let mut push_constants = [0u8; PUSH_CONSTANTS_SIZE as usize];
    write_matrix(&mut push_constants[..64], &pass.light);
    let mut bound_buffers = None;
    for draw in &pass.draws {
        write_matrix(&mut push_constants[64..], &draw.model);
        device.cmd_push_constants(
            command_buffer,
            renderer.layout,
            vk::ShaderStageFlags::VERTEX,
            0,
            &push_constants,
        );
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
}

/// Writes `matrix` column by column as 16 floats.
fn write_matrix(bytes: &mut [u8], matrix: &Mat4) {
    let values: &[f32; 16] = matrix.as_ref();
    for (chunk, value) in bytes.chunks_exact_mut(4).zip(values) {
        chunk.copy_from_slice(&value.to_ne_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cgmath::SquareMatrix;

    #[test]
    fn the_shadow_shader_is_valid_spirv() {
        assert!(Bytecode::new(SHADOW_VERT).is_ok());
    }

    #[test]
    fn matrices_are_written_column_by_column() {
        let mut bytes = [0u8; 64];
        let m = Mat4::from_translation(crate::math::Vec3::new(1.0, 2.0, 3.0));
        write_matrix(&mut bytes, &m);
        let float = |bytes: &[u8], i: usize| {
            f32::from_ne_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap())
        };
        let floats = [0, 12, 13, 14].map(|i| float(&bytes, i));
        assert_eq!(floats, [1.0, 1.0, 2.0, 3.0]);
        write_matrix(&mut bytes, &Mat4::identity());
        assert_eq!(float(&bytes, 12), 0.0);
    }
}
