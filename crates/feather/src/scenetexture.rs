//! Scene textures on the GPU: the image a texture scene is drawn into every frame, its render
//! pass and framebuffer, and recording the pass and the mipmaps. See `Atlas::scene_texture` and
//! `FeatherApp::get_texture_scene`.
//!
//! Each frame, before the window's render pass: the texture scene's draws go into the
//! texture's own render pass (multisampled and resolved, or with one sample), which leaves
//! mip level 0 ready to be blitted; then every other level is blitted from the one above, and
//! every level is left ready for fragment shaders to sample.

use std::ops::Range;

use anyhow::Result;
use vulkanalia::prelude::v1_0::*;

use super::atlas::ColorSpace;
use super::commandbuffers::{record_draws, GpuDraw};
use super::gpuimage::image_format;
use super::images::{create_image, create_image_view};

/// One scene texture on the GPU, with everything to draw into it.
pub struct SceneTextureGpu {
    pub width: u32,
    pub height: u32,
    pub samples: vk::SampleCountFlags,
    pub mip_levels: u32,
    pub render_pass: vk::RenderPass,
    pub framebuffer: vk::Framebuffer,
    /// The texture: every mip level, sampled by shaders.
    image: vk::Image,
    memory: vk::DeviceMemory,
    pub view: vk::ImageView,
    /// Mip level 0 only: drawn into (or resolved into).
    level0_view: vk::ImageView,
    /// The multisampled colour image, when `samples` is more than 1.
    msaa: Option<(vk::Image, vk::DeviceMemory, vk::ImageView)>,
    depth: (vk::Image, vk::DeviceMemory, vk::ImageView),
}

impl SceneTextureGpu {
    /// Creates a `width` x `height` scene texture drawn with `samples` samples per pixel, its
    /// depth in `depth_format`.
    pub unsafe fn create(
        instance: &Instance,
        device: &Device,
        physical_device: vk::PhysicalDevice,
        width: u32,
        height: u32,
        samples: vk::SampleCountFlags,
        depth_format: vk::Format,
    ) -> Result<Self> {
        let format = image_format(ColorSpace::Srgb);
        // Mipmaps are made by blitting, which needs linear filtering of the format.
        let can_blit = instance
            .get_physical_device_format_properties(physical_device, format)
            .optimal_tiling_features
            .contains(
                vk::FormatFeatureFlags::SAMPLED_IMAGE_FILTER_LINEAR
                    | vk::FormatFeatureFlags::BLIT_SRC
                    | vk::FormatFeatureFlags::BLIT_DST,
            );
        let mip_levels = if can_blit {
            32 - width.max(height).leading_zeros()
        } else {
            1
        };

        let mut texture = Self {
            width,
            height,
            samples,
            mip_levels,
            render_pass: vk::RenderPass::null(),
            framebuffer: vk::Framebuffer::null(),
            image: vk::Image::null(),
            memory: vk::DeviceMemory::null(),
            view: vk::ImageView::null(),
            level0_view: vk::ImageView::null(),
            msaa: None,
            depth: (
                vk::Image::null(),
                vk::DeviceMemory::null(),
                vk::ImageView::null(),
            ),
        };
        let result = (|| -> Result<()> {
            let (image, memory) = create_image(
                instance,
                device,
                physical_device,
                width,
                height,
                mip_levels,
                vk::SampleCountFlags::_1,
                format,
                vk::ImageTiling::OPTIMAL,
                vk::ImageUsageFlags::COLOR_ATTACHMENT
                    | vk::ImageUsageFlags::SAMPLED
                    | vk::ImageUsageFlags::TRANSFER_SRC
                    | vk::ImageUsageFlags::TRANSFER_DST,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            )?;
            texture.image = image;
            texture.memory = memory;
            texture.view = create_image_view(
                device,
                image,
                format,
                vk::ImageAspectFlags::COLOR,
                mip_levels,
            )?;
            texture.level0_view =
                create_image_view(device, image, format, vk::ImageAspectFlags::COLOR, 1)?;

            // Attachments only used within the pass: transient, lazily allocated where the
            // device can.
            let transient = |format, usage, aspect| -> Result<_> {
                let (image, memory) = create_image(
                    instance,
                    device,
                    physical_device,
                    width,
                    height,
                    1,
                    samples,
                    format,
                    vk::ImageTiling::OPTIMAL,
                    usage | vk::ImageUsageFlags::TRANSIENT_ATTACHMENT,
                    vk::MemoryPropertyFlags::DEVICE_LOCAL,
                )?;
                let view = create_image_view(device, image, format, aspect, 1);
                match view {
                    Ok(view) => Ok((image, memory, view)),
                    Err(e) => {
                        device.destroy_image(image, None);
                        device.free_memory(memory, None);
                        Err(e)
                    }
                }
            };
            if samples != vk::SampleCountFlags::_1 {
                texture.msaa = Some(transient(
                    format,
                    vk::ImageUsageFlags::COLOR_ATTACHMENT,
                    vk::ImageAspectFlags::COLOR,
                )?);
            }
            texture.depth = transient(
                depth_format,
                vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                vk::ImageAspectFlags::DEPTH,
            )?;

            texture.render_pass =
                create_render_pass(device, format, depth_format, samples, mip_levels > 1)?;
            let attachments: Vec<vk::ImageView> = match texture.msaa {
                Some((_, _, msaa_view)) => vec![msaa_view, texture.depth.2, texture.level0_view],
                None => vec![texture.level0_view, texture.depth.2],
            };
            let info = vk::FramebufferCreateInfo::builder()
                .render_pass(texture.render_pass)
                .attachments(&attachments)
                .width(width)
                .height(height)
                .layers(1);
            texture.framebuffer = device.create_framebuffer(&info, None)?;
            Ok(())
        })();
        if let Err(e) = result {
            texture.destroy(device);
            return Err(e);
        }
        Ok(texture)
    }

    pub fn extent(&self) -> vk::Extent2D {
        vk::Extent2D {
            width: self.width,
            height: self.height,
        }
    }

    /// The GPU must not be using it.
    pub unsafe fn destroy(&self, device: &Device) {
        if self.framebuffer != vk::Framebuffer::null() {
            device.destroy_framebuffer(self.framebuffer, None);
        }
        if self.render_pass != vk::RenderPass::null() {
            device.destroy_render_pass(self.render_pass, None);
        }
        for (image, memory, view) in self.msaa.iter().chain([&self.depth]) {
            if *view != vk::ImageView::null() {
                device.destroy_image_view(*view, None);
            }
            if *image != vk::Image::null() {
                device.destroy_image(*image, None);
            }
            if *memory != vk::DeviceMemory::null() {
                device.free_memory(*memory, None);
            }
        }
        for view in [self.level0_view, self.view] {
            if view != vk::ImageView::null() {
                device.destroy_image_view(view, None);
            }
        }
        if self.image != vk::Image::null() {
            device.destroy_image(self.image, None);
        }
        if self.memory != vk::DeviceMemory::null() {
            device.free_memory(self.memory, None);
        }
    }
}

/// The render pass of a scene texture. With more than one sample: a multisampled colour
/// attachment, resolved into mip level 0; otherwise level 0 is drawn into directly. Level 0 is
/// left ready to be blitted into the other levels (`mipmaps`), or to be sampled.
unsafe fn create_render_pass(
    device: &Device,
    format: vk::Format,
    depth_format: vk::Format,
    samples: vk::SampleCountFlags,
    mipmaps: bool,
) -> Result<vk::RenderPass> {
    let final_layout = if mipmaps {
        vk::ImageLayout::TRANSFER_SRC_OPTIMAL
    } else {
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
    };
    let multisampled = samples != vk::SampleCountFlags::_1;
    let color = vk::AttachmentDescription::builder()
        .format(format)
        .samples(samples)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        // Multisampled: only the resolve is kept.
        .store_op(if multisampled {
            vk::AttachmentStoreOp::DONT_CARE
        } else {
            vk::AttachmentStoreOp::STORE
        })
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(if multisampled {
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL
        } else {
            final_layout
        })
        .build();
    let depth = vk::AttachmentDescription::builder()
        .format(depth_format)
        .samples(samples)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::DONT_CARE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
        .build();
    let resolve = vk::AttachmentDescription::builder()
        .format(format)
        .samples(vk::SampleCountFlags::_1)
        .load_op(vk::AttachmentLoadOp::DONT_CARE)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(final_layout)
        .build();

    let color_ref = &[vk::AttachmentReference::builder()
        .attachment(0)
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
        .build()];
    let depth_ref = vk::AttachmentReference::builder()
        .attachment(1)
        .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
    let resolve_ref = &[vk::AttachmentReference::builder()
        .attachment(2)
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
        .build()];
    let mut subpass = vk::SubpassDescription::builder()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(color_ref)
        .depth_stencil_attachment(&depth_ref);
    if multisampled {
        subpass = subpass.resolve_attachments(resolve_ref);
    }

    let dependencies = &[
        // The previous frame read the texture (fragment shaders, mipmap blits) and wrote the
        // attachments: wait for that before writing them again.
        vk::SubpassDependency::builder()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(
                vk::PipelineStageFlags::FRAGMENT_SHADER
                    | vk::PipelineStageFlags::TRANSFER
                    | vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                    | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
            )
            .src_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE
                    | vk::AccessFlags::TRANSFER_WRITE,
            )
            .dst_stage_mask(
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                    | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
            )
            .dst_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            )
            .build(),
        // Level 0 is then blitted (mipmaps) or sampled by the window's pass.
        vk::SubpassDependency::builder()
            .src_subpass(0)
            .dst_subpass(vk::SUBPASS_EXTERNAL)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .dst_stage_mask(
                vk::PipelineStageFlags::TRANSFER | vk::PipelineStageFlags::FRAGMENT_SHADER,
            )
            .dst_access_mask(vk::AccessFlags::TRANSFER_READ | vk::AccessFlags::SHADER_READ)
            .build(),
    ];

    let all = [color, depth, resolve];
    let attachments = if multisampled { &all[..] } else { &all[..2] };
    let subpasses = &[subpass];
    let info = vk::RenderPassCreateInfo::builder()
        .attachments(attachments)
        .subpasses(subpasses)
        .dependencies(dependencies);
    Ok(device.create_render_pass(&info, None)?)
}

/// One texture scene's pass in a frame: its draws (a range of the frame's draws) into a scene
/// texture.
pub struct TexturePass {
    pub render_pass: vk::RenderPass,
    pub framebuffer: vk::Framebuffer,
    pub extent: vk::Extent2D,
    pub image: vk::Image,
    pub mip_levels: u32,
    pub draws: Range<usize>,
}

impl SceneTextureGpu {
    /// The pass drawing `draws` into this texture.
    pub fn pass(&self, draws: Range<usize>) -> TexturePass {
        TexturePass {
            render_pass: self.render_pass,
            framebuffer: self.framebuffer,
            extent: self.extent(),
            image: self.image,
            mip_levels: self.mip_levels,
            draws,
        }
    }
}

/// Records `pass` into `command_buffer`, outside any render pass: the texture scene's draws,
/// then the mipmaps.
pub unsafe fn record_texture_pass(
    device: &Device,
    command_buffer: vk::CommandBuffer,
    pass: &TexturePass,
    draws: &[GpuDraw],
) {
    let render_area = vk::Rect2D::builder()
        .offset(vk::Offset2D::default())
        .extent(pass.extent)
        .build();
    let clear_values = &[
        vk::ClearValue {
            color: vk::ClearColorValue {
                float32: [0.0, 0.0, 0.0, 1.0],
            },
        },
        vk::ClearValue {
            depth_stencil: vk::ClearDepthStencilValue {
                depth: 1.0,
                stencil: 0,
            },
        },
        // The resolve attachment, if any, isn't cleared.
        vk::ClearValue::default(),
    ];
    let info = vk::RenderPassBeginInfo::builder()
        .render_pass(pass.render_pass)
        .framebuffer(pass.framebuffer)
        .render_area(render_area)
        .clear_values(clear_values);
    device.cmd_begin_render_pass(command_buffer, &info, vk::SubpassContents::INLINE);
    record_draws(
        device,
        command_buffer,
        &draws[pass.draws.clone()],
        render_area,
    );
    device.cmd_end_render_pass(command_buffer);

    if pass.mip_levels > 1 {
        record_mipmaps(device, command_buffer, pass);
    }
}

/// Blits mip levels 1.. from level 0 (in `TRANSFER_SRC_OPTIMAL`, as the pass leaves it), each
/// from the one above, and leaves every level in `SHADER_READ_ONLY_OPTIMAL`.
unsafe fn record_mipmaps(device: &Device, command_buffer: vk::CommandBuffer, pass: &TexturePass) {
    let range = |base: u32, count: u32| {
        vk::ImageSubresourceRange::builder()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .base_mip_level(base)
            .level_count(count)
            .base_array_layer(0)
            .layer_count(1)
            .build()
    };
    let barrier = |range, old, new, src_access, dst_access| {
        vk::ImageMemoryBarrier::builder()
            .image(pass.image)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .subresource_range(range)
            .old_layout(old)
            .new_layout(new)
            .src_access_mask(src_access)
            .dst_access_mask(dst_access)
            .build()
    };
    let pipeline_barrier = |src_stage, dst_stage, barrier: vk::ImageMemoryBarrier| {
        device.cmd_pipeline_barrier(
            command_buffer,
            src_stage,
            dst_stage,
            vk::DependencyFlags::empty(),
            &[] as &[vk::MemoryBarrier],
            &[] as &[vk::BufferMemoryBarrier],
            &[barrier],
        );
    };

    // Levels 1..: their old contents (sampled by the previous frame) are dropped.
    pipeline_barrier(
        vk::PipelineStageFlags::FRAGMENT_SHADER,
        vk::PipelineStageFlags::TRANSFER,
        barrier(
            range(1, pass.mip_levels - 1),
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::AccessFlags::empty(),
            vk::AccessFlags::TRANSFER_WRITE,
        ),
    );

    let layers = |level| {
        vk::ImageSubresourceLayers::builder()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .mip_level(level)
            .base_array_layer(0)
            .layer_count(1)
            .build()
    };
    let (mut width, mut height) = (pass.extent.width as i32, pass.extent.height as i32);
    for level in 1..pass.mip_levels {
        if level > 1 {
            // The level above was just written: make it the blit's source.
            pipeline_barrier(
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::TRANSFER,
                barrier(
                    range(level - 1, 1),
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    vk::AccessFlags::TRANSFER_WRITE,
                    vk::AccessFlags::TRANSFER_READ,
                ),
            );
        }
        let (next_width, next_height) = ((width / 2).max(1), (height / 2).max(1));
        let blit = vk::ImageBlit::builder()
            .src_offsets([
                vk::Offset3D { x: 0, y: 0, z: 0 },
                vk::Offset3D {
                    x: width,
                    y: height,
                    z: 1,
                },
            ])
            .src_subresource(layers(level - 1))
            .dst_offsets([
                vk::Offset3D { x: 0, y: 0, z: 0 },
                vk::Offset3D {
                    x: next_width,
                    y: next_height,
                    z: 1,
                },
            ])
            .dst_subresource(layers(level))
            .build();
        device.cmd_blit_image(
            command_buffer,
            pass.image,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            pass.image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &[blit],
            vk::Filter::LINEAR,
        );
        // The level above is done: ready to be sampled.
        pipeline_barrier(
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::FRAGMENT_SHADER,
            barrier(
                range(level - 1, 1),
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                vk::AccessFlags::TRANSFER_READ,
                vk::AccessFlags::SHADER_READ,
            ),
        );
        (width, height) = (next_width, next_height);
    }
    // The last level was only written.
    pipeline_barrier(
        vk::PipelineStageFlags::TRANSFER,
        vk::PipelineStageFlags::FRAGMENT_SHADER,
        barrier(
            range(pass.mip_levels - 1, 1),
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            vk::AccessFlags::TRANSFER_WRITE,
            vk::AccessFlags::SHADER_READ,
        ),
    );
}
