use anyhow::{anyhow, Result};
use vulkanalia::prelude::v1_0::*;

//================================================
// Shared (Other)
//================================================

pub unsafe fn get_memory_type_index(
    instance: &Instance,
    physical_device: &vk::PhysicalDevice,
    properties: vk::MemoryPropertyFlags,
    requirements: vk::MemoryRequirements,
) -> Result<u32> {
    get_memory_type_index_preferring(instance, physical_device, properties, properties, requirements)
}

/// A memory type for `requirements` with the `preferred` properties if the device has one,
/// otherwise one with the `required` properties.
pub unsafe fn get_memory_type_index_preferring(
    instance: &Instance,
    physical_device: &vk::PhysicalDevice,
    preferred: vk::MemoryPropertyFlags,
    required: vk::MemoryPropertyFlags,
    requirements: vk::MemoryRequirements,
) -> Result<u32> {
    let memory = instance.get_physical_device_memory_properties(*physical_device);
    let types = &memory.memory_types[..memory.memory_type_count as usize];
    choose_memory_type(types, requirements.memory_type_bits, preferred, required)
        .ok_or_else(|| anyhow!("Failed to find suitable memory type."))
}

/// The first of `types` allowed by `type_bits` with the `preferred` properties, or else the
/// first with the `required` ones.
fn choose_memory_type(
    types: &[vk::MemoryType],
    type_bits: u32,
    preferred: vk::MemoryPropertyFlags,
    required: vk::MemoryPropertyFlags,
) -> Option<u32> {
    let find = |properties: vk::MemoryPropertyFlags| {
        (0..types.len() as u32).find(|&i| {
            type_bits & (1 << i) != 0 && types[i as usize].property_flags.contains(properties)
        })
    };
    find(preferred).or_else(|| find(required))
}

pub unsafe fn begin_single_time_commands(
    device: &Device,
    command_pool: &vk::CommandPool,
) -> Result<vk::CommandBuffer> {
    // Allocate

    let info = vk::CommandBufferAllocateInfo::builder()
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_pool(*command_pool)
        .command_buffer_count(1);

    let command_buffer = device.allocate_command_buffers(&info)?[0];

    // Begin

    let info =
        vk::CommandBufferBeginInfo::builder().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

    device.begin_command_buffer(command_buffer, &info)?;

    Ok(command_buffer)
}

pub unsafe fn end_single_time_commands(
    device: &Device,
    command_pool: &vk::CommandPool,
    graphics_queue: &vk::Queue,
    command_buffer: vk::CommandBuffer,
) -> Result<()> {
    // End

    device.end_command_buffer(command_buffer)?;

    // Submit

    let command_buffers = &[command_buffer];
    let info = vk::SubmitInfo::builder().command_buffers(command_buffers);

    device.queue_submit(*graphics_queue, &[info], vk::Fence::null())?;
    device.queue_wait_idle(*graphics_queue)?;

    // Cleanup

    device.free_command_buffers(*command_pool, &[command_buffer]);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_type(property_flags: vk::MemoryPropertyFlags) -> vk::MemoryType {
        vk::MemoryType {
            property_flags,
            heap_index: 0,
        }
    }

    #[test]
    fn preferred_memory_type_first_then_required() {
        let local = vk::MemoryPropertyFlags::DEVICE_LOCAL;
        let lazy = local | vk::MemoryPropertyFlags::LAZILY_ALLOCATED;
        let host = vk::MemoryPropertyFlags::HOST_VISIBLE;
        let types = [memory_type(host), memory_type(local), memory_type(lazy)];
        // A tiled GPU: the lazily allocated type is taken when the image allows it.
        assert_eq!(choose_memory_type(&types, 0b111, lazy, local), Some(2));
        // Not allowed for this image: falls back to device-local memory.
        assert_eq!(choose_memory_type(&types, 0b011, lazy, local), Some(1));
        // A desktop GPU without lazily allocated memory.
        assert_eq!(choose_memory_type(&types[..2], 0b11, lazy, local), Some(1));
        // Nothing suitable.
        assert_eq!(choose_memory_type(&types, 0b001, lazy, local), None);
    }
}
