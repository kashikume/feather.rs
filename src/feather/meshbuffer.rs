use std::mem::size_of;
use std::ptr::copy_nonoverlapping;

use anyhow::Result;

use vulkanalia::prelude::v1_0::*;
use vulkanalia::vk;
use vulkanalia::Device;
use vulkanalia::Instance;

use crate::feather::appdata::AppData;
use crate::feather::buffers::copy_buffer;
use crate::feather::buffers::create_buffer;
use crate::feather::meshbufferdata::MeshBufferData;
use crate::feather::scene::Scene;
use crate::feather::vertex::Vertex;

use super::mesh::Mesh;
use super::object::Object;

#[derive(Default)]
pub struct MeshBuffer {
    handle: usize,
    num_vertexes: usize,
    num_indexes: usize,
    mesh_handles: Vec<usize>,
    pub vertex_buffer: vk::Buffer,
    pub vertex_buffer_memory: vk::DeviceMemory,
    pub index_buffer: vk::Buffer,
    pub index_buffer_memory: vk::DeviceMemory,
}

impl Object for MeshBuffer {
    fn get_name(&self) -> Option<String> {
        None
    }

    fn set_handle(&mut self, handle: usize) {
        self.handle = handle;
    }

    fn get_handle(&self) -> usize {
        self.handle
    }
}

impl MeshBuffer {
    pub fn new() -> Self {
        Self {
            handle: 0,
            num_vertexes: 0,
            num_indexes: 0,
            mesh_handles: Vec::new(),
            vertex_buffer: vk::Buffer::default(),
            vertex_buffer_memory: vk::DeviceMemory::default(),
            index_buffer: vk::Buffer::default(),
            index_buffer_memory: vk::DeviceMemory::default(),
        }
    }

    pub fn add_mesh(&mut self, mesh: &mut Mesh) {
        self.mesh_handles.push(mesh.get_handle());
        let mesh_buffer_data = MeshBufferData::new(
            self.handle,
            self.num_vertexes,
            mesh.gen_num_vertexes(),
            self.num_indexes,
            mesh.gen_num_indexes(),
        );
        mesh.set_mesh_buffer_data(mesh_buffer_data);
        self.num_vertexes += mesh.gen_num_vertexes();
        self.num_indexes += mesh.gen_num_indexes();
    }

    pub unsafe fn data_size_for_vertexes(&self) -> usize {
        self.num_vertexes * size_of::<Vertex>()
    }

    pub unsafe fn data_size_for_indexes(&self) -> usize {
        self.num_indexes * size_of::<u32>()
    }

    pub unsafe fn create_vertex_buffer(
        &mut self,
        instance: &Instance,
        device: &Device,
        data: &mut AppData,
        scene: &Scene,
    ) -> Result<()> {
        // Create (staging)

        let size = self.data_size_for_vertexes() as u64;

        let (staging_buffer, staging_buffer_memory) = create_buffer(
            instance,
            device,
            &data.physical_device,
            size,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_COHERENT | vk::MemoryPropertyFlags::HOST_VISIBLE,
        )?;

        // Copy (staging)

        let memory =
            device.map_memory(staging_buffer_memory, 0, size, vk::MemoryMapFlags::empty())?;

        let mut offset = 0usize;
        for mesh_handle in &self.mesh_handles {
            let mesh = scene.get_mesh(*mesh_handle).unwrap();
            copy_nonoverlapping(
                mesh.vertices.as_ptr(),
                memory.cast::<crate::feather::vertex::Vertex>().add(offset),
                mesh.gen_num_vertexes(),
            );
            offset += mesh.gen_num_vertexes();
        }

        device.unmap_memory(staging_buffer_memory);

        // Create (vertex)

        let (vertex_buffer, vertex_buffer_memory) = create_buffer(
            instance,
            device,
            &data.physical_device,
            size,
            vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::VERTEX_BUFFER,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;

        self.vertex_buffer = vertex_buffer;
        self.vertex_buffer_memory = vertex_buffer_memory;

        // Copy (vertex)

        copy_buffer(
            device,
            &data.command_pool,
            &data.graphics_queue,
            staging_buffer,
            vertex_buffer,
            size,
        )?;

        // Cleanup

        device.destroy_buffer(staging_buffer, None);
        device.free_memory(staging_buffer_memory, None);

        Ok(())
    }

    pub unsafe fn create_index_buffer(
        &mut self,
        instance: &Instance,
        device: &Device,
        data: &mut AppData,
        scene: &Scene,
    ) -> Result<()> {
        // Create (staging)

        let size = self.data_size_for_indexes() as u64;

        let (staging_buffer, staging_buffer_memory) = create_buffer(
            instance,
            device,
            &data.physical_device,
            size,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_COHERENT | vk::MemoryPropertyFlags::HOST_VISIBLE,
        )?;

        // Copy (staging)

        let memory =
            device.map_memory(staging_buffer_memory, 0, size, vk::MemoryMapFlags::empty())?;

        let mut offset = 0usize;
        for mesh_handle in &self.mesh_handles {
            let mesh = scene.get_mesh(*mesh_handle).unwrap();
            copy_nonoverlapping(
                mesh.indices.as_ptr(),
                memory.cast::<u32>().add(offset),
                mesh.gen_num_indexes(),
            );
            offset += mesh.gen_num_indexes();
        }

        device.unmap_memory(staging_buffer_memory);

        // Create (index)

        let (index_buffer, index_buffer_memory) = create_buffer(
            instance,
            device,
            &data.physical_device,
            size,
            vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::INDEX_BUFFER,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;

        self.index_buffer = index_buffer;
        self.index_buffer_memory = index_buffer_memory;

        // Copy (index)

        copy_buffer(
            device,
            &data.command_pool,
            &data.graphics_queue,
            staging_buffer,
            index_buffer,
            size,
        )?;

        // Cleanup

        device.destroy_buffer(staging_buffer, None);
        device.free_memory(staging_buffer_memory, None);

        Ok(())
    }

    pub unsafe fn cleanup(&mut self, device: &Device) {
        device.destroy_buffer(self.vertex_buffer, None);
        device.free_memory(self.vertex_buffer_memory, None);
        device.destroy_buffer(self.index_buffer, None);
        device.free_memory(self.index_buffer_memory, None);
    }
}
