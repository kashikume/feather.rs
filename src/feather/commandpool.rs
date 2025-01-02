use anyhow::Result;
use vulkanalia::prelude::v1_0::*;

use super::appdata::AppData;
use super::queuefamilyindices::QueueFamilyIndices;

//================================================
// Command Pool
//================================================

pub unsafe fn create_command_pool(
    instance: &Instance,
    device: &Device,
    data: &mut AppData,
) -> Result<()> {
    let indices = QueueFamilyIndices::get(instance, data.surface, data.physical_device)?;

    let info = vk::CommandPoolCreateInfo::builder().queue_family_index(indices.graphics);

    data.command_pool = device.create_command_pool(&info, None)?;

    Ok(())
}
