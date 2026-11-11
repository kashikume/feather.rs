use std::ptr::copy_nonoverlapping as memcpy;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;
use vulkanalia::prelude::v1_0::*;
use vulkanalia::vk::PhysicalDevice;

use super::other::{begin_single_time_commands, end_single_time_commands, get_memory_type_index};

//================================================
// Buffers
//================================================

/// A host-visible uniform buffer, mapped for its whole life. Each swapchain image has one,
/// holding the uniform blocks of every draw of its frame.
pub struct FrameUniformBuffer {
    pub buffer: vk::Buffer,
    /// Tells this buffer apart from every other one, for the lifetime of the process. The
    /// `buffer` handle can't: a driver may give a new buffer the handle of one just destroyed
    /// (Android drivers do), and descriptor sets written for the old one must be rewritten.
    pub id: u64,
    memory: vk::DeviceMemory,
    pub size: u64,
    mapped: *mut u8,
}

impl FrameUniformBuffer {
    /// Smallest buffer created, so that small scenes don't regrow it frame after frame.
    const MIN_SIZE: u64 = 64 * 1024;

    pub unsafe fn create(
        instance: &Instance,
        device: &Device,
        physical_device: &PhysicalDevice,
        min_size: u64,
    ) -> Result<Self> {
        let size = min_size.max(Self::MIN_SIZE).next_power_of_two();
        let (buffer, memory) = create_buffer(
            instance,
            device,
            physical_device,
            size,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            vk::MemoryPropertyFlags::HOST_COHERENT | vk::MemoryPropertyFlags::HOST_VISIBLE,
        )?;
        let mapped = match device.map_memory(memory, 0, size, vk::MemoryMapFlags::empty()) {
            Ok(mapped) => mapped.cast(),
            Err(e) => {
                device.destroy_buffer(buffer, None);
                device.free_memory(memory, None);
                return Err(e.into());
            }
        };
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Ok(Self {
            buffer,
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            memory,
            size,
            mapped,
        })
    }

    /// Copies `bytes` to the start of the buffer. Must fit, and the buffer must not be in use.
    pub unsafe fn write(&self, bytes: &[u8]) {
        assert!(bytes.len() as u64 <= self.size);
        memcpy(bytes.as_ptr(), self.mapped, bytes.len());
    }

    pub unsafe fn destroy(&self, device: &Device) {
        device.unmap_memory(self.memory);
        device.destroy_buffer(self.buffer, None);
        device.free_memory(self.memory, None);
    }
}

//================================================
// Shared (Buffers)
//================================================

pub unsafe fn create_buffer(
    instance: &Instance,
    device: &Device,
    physical_device: &PhysicalDevice,
    size: vk::DeviceSize,
    usage: vk::BufferUsageFlags,
    properties: vk::MemoryPropertyFlags,
) -> Result<(vk::Buffer, vk::DeviceMemory)> {
    // Buffer

    let buffer_info = vk::BufferCreateInfo::builder()
        .size(size)
        .usage(usage)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);

    let buffer = device.create_buffer(&buffer_info, None)?;

    // Memory

    let requirements = device.get_buffer_memory_requirements(buffer);

    let memory_info = vk::MemoryAllocateInfo::builder()
        .allocation_size(requirements.size)
        .memory_type_index(get_memory_type_index(
            instance,
            physical_device,
            properties,
            requirements,
        )?);

    let buffer_memory = device.allocate_memory(&memory_info, None)?;

    device.bind_buffer_memory(buffer, buffer_memory, 0)?;

    Ok((buffer, buffer_memory))
}

pub unsafe fn copy_buffer(
    device: &Device,
    command_pool: &vk::CommandPool,
    graphics_queue: &vk::Queue,
    source: vk::Buffer,
    destination: vk::Buffer,
    size: vk::DeviceSize,
) -> Result<()> {
    let command_buffer = begin_single_time_commands(device, command_pool)?;

    let regions = vk::BufferCopy::builder().size(size);
    device.cmd_copy_buffer(command_buffer, source, destination, &[regions]);

    end_single_time_commands(device, command_pool, graphics_queue, command_buffer)?;

    Ok(())
}
