use anyhow::Result;
use vulkanalia::prelude::v1_0::*;

use super::appdata::AppData;
use super::images::{create_image, create_image_view};

//================================================
// Color Objects
//================================================

pub unsafe fn create_color_objects(
    instance: &Instance,
    device: &Device,
    data: &mut AppData,
) -> Result<()> {
    // Image + Image Memory

    let (color_image, color_image_memory) = create_image(
        instance,
        device,
        data,
        data.swapchain.swapchain_extent.width,
        data.swapchain.swapchain_extent.height,
        1,
        data.msaa_samples,
        data.swapchain.swapchain_format,
        vk::ImageTiling::OPTIMAL,
        vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSIENT_ATTACHMENT,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )?;

    data.color_image = color_image;
    data.color_image_memory = color_image_memory;

    // Image View

    data.color_image_view = create_image_view(
        device,
        data.color_image,
        data.swapchain.swapchain_format,
        vk::ImageAspectFlags::COLOR,
        1,
    )?;

    Ok(())
}
