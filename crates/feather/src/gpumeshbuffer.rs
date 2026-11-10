use std::ptr::copy_nonoverlapping;

use anyhow::{bail, Result};
use vulkanalia::prelude::v1_0::*;

use super::buffers::{copy_buffer, create_buffer};
use super::gpuimage::UploadContext;
use super::math::{Vec2, Vec3};
use super::mesh::Mesh;
use super::meshbuffer::MeshBuffer;
use super::objdb::ObjDB;
use super::vertex::Vertex;

/// A `MeshBuffer` on the GPU: device-local vertex and index buffers.
pub struct GpuMeshBuffer {
    /// `MeshBuffer::get_id` of the uploaded buffer.
    pub id: u64,
    pub vertex_buffer: vk::Buffer,
    vertex_memory: vk::DeviceMemory,
    pub index_buffer: vk::Buffer,
    index_memory: vk::DeviceMemory,
}

impl GpuMeshBuffer {
    /// Uploads the vertices and indices of the meshes in `buffer`. Slots of removed meshes are
    /// filled with zeros, so the other meshes stay where `MeshBufferData` says they are.
    pub unsafe fn upload(
        ctx: &UploadContext,
        buffer: &MeshBuffer,
        meshes: &ObjDB<Mesh>,
    ) -> Result<Self> {
        let mut vertices: Vec<Vertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        for slot in buffer.get_slots() {
            let Some(handle) = slot.mesh else {
                let zero = Vertex::new(
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec2::new(0.0, 0.0),
                );
                vertices.resize(vertices.len() + slot.num_vertexes, zero);
                indices.resize(indices.len() + slot.num_indexes, 0);
                continue;
            };
            let Some(mesh) = meshes.get(handle) else {
                bail!("mesh {handle} of the mesh buffer doesn't exist");
            };
            vertices.extend_from_slice(&mesh.vertices);
            indices.extend_from_slice(&mesh.indices);
        }
        if buffer.is_empty() || vertices.is_empty() || indices.is_empty() {
            bail!("the mesh buffer is empty");
        }

        let (vertex_buffer, vertex_memory) =
            upload_slice(ctx, &vertices, vk::BufferUsageFlags::VERTEX_BUFFER)?;
        let (index_buffer, index_memory) =
            match upload_slice(ctx, &indices, vk::BufferUsageFlags::INDEX_BUFFER) {
                Ok(index) => index,
                Err(e) => {
                    ctx.device.destroy_buffer(vertex_buffer, None);
                    ctx.device.free_memory(vertex_memory, None);
                    return Err(e);
                }
            };
        Ok(Self {
            id: buffer.get_id(),
            vertex_buffer,
            vertex_memory,
            index_buffer,
            index_memory,
        })
    }

    pub unsafe fn destroy(&self, device: &Device) {
        device.destroy_buffer(self.vertex_buffer, None);
        device.free_memory(self.vertex_memory, None);
        device.destroy_buffer(self.index_buffer, None);
        device.free_memory(self.index_memory, None);
    }
}

/// Copies `data` into a new device-local buffer through a staging buffer.
unsafe fn upload_slice<T: Copy>(
    ctx: &UploadContext,
    data: &[T],
    usage: vk::BufferUsageFlags,
) -> Result<(vk::Buffer, vk::DeviceMemory)> {
    let device = ctx.device;
    let size = std::mem::size_of_val(data) as u64;

    let (staging_buffer, staging_memory) = create_buffer(
        ctx.instance,
        device,
        &ctx.physical_device,
        size,
        vk::BufferUsageFlags::TRANSFER_SRC,
        vk::MemoryPropertyFlags::HOST_COHERENT | vk::MemoryPropertyFlags::HOST_VISIBLE,
    )?;

    let result = (|| {
        let memory = device.map_memory(staging_memory, 0, size, vk::MemoryMapFlags::empty())?;
        copy_nonoverlapping(data.as_ptr(), memory.cast(), data.len());
        device.unmap_memory(staging_memory);

        let (buffer, memory) = create_buffer(
            ctx.instance,
            device,
            &ctx.physical_device,
            size,
            vk::BufferUsageFlags::TRANSFER_DST | usage,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;
        if let Err(e) = copy_buffer(
            device,
            &ctx.command_pool,
            &ctx.queue,
            staging_buffer,
            buffer,
            size,
        ) {
            device.destroy_buffer(buffer, None);
            device.free_memory(memory, None);
            return Err(e);
        }
        Ok((buffer, memory))
    })();

    device.destroy_buffer(staging_buffer, None);
    device.free_memory(staging_memory, None);
    result
}
