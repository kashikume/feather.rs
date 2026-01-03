use std::mem::size_of;
use std::ptr::copy_nonoverlapping;

use anyhow::Result;

use vulkanalia::prelude::v1_0::*;
use vulkanalia::vk;
use vulkanalia::vk::CommandPool;
use vulkanalia::vk::PhysicalDevice;
use vulkanalia::vk::Queue;
use vulkanalia::Device;
use vulkanalia::Instance;

use crate::feather::buffers::copy_buffer;
use crate::feather::buffers::create_buffer;
use crate::feather::meshbufferdata::MeshBufferData;
use crate::feather::scene::Scene;
use crate::feather::vertex::Vertex;

use super::mesh::Mesh;
use super::objdb::ObjDB;
use super::object::Object;

#[derive(Default)]
pub struct MeshBuffer {
    handle: usize,
    num_vertexes: usize,
    num_indexes: usize,
    mesh_handles: Vec<usize>,
    pub vertex_buffer: Option<vk::Buffer>,
    pub vertex_buffer_memory: Option<vk::DeviceMemory>,
    pub index_buffer: Option<vk::Buffer>,
    pub index_buffer_memory: Option<vk::DeviceMemory>,
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
            vertex_buffer: None,
            vertex_buffer_memory: None,
            index_buffer: None,
            index_buffer_memory: None,
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

    pub fn data_size_for_vertexes(&self) -> usize {
        self.num_vertexes * size_of::<Vertex>()
    }

    pub fn data_size_for_indexes(&self) -> usize {
        self.num_indexes * size_of::<u32>()
    }

    pub unsafe fn create_vertex_buffer(
        &mut self,
        instance: &Instance,
        device: &Device,
        physical_device: &PhysicalDevice,
        command_pool: &CommandPool,
        graphics_queue: &Queue,
        meshes: &ObjDB<Mesh>,
    ) -> Result<()> {
        // Create (staging)

        let size = self.data_size_for_vertexes() as u64;

        let (staging_buffer, staging_buffer_memory) = create_buffer(
            instance,
            device,
            physical_device,
            size,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_COHERENT | vk::MemoryPropertyFlags::HOST_VISIBLE,
        )?;

        // Copy (staging)

        let memory =
            device.map_memory(staging_buffer_memory, 0, size, vk::MemoryMapFlags::empty())?;

        let mut offset = 0usize;
        for mesh_handle in &self.mesh_handles {
            let mesh = meshes.get(*mesh_handle).unwrap();
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
            physical_device,
            size,
            vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::VERTEX_BUFFER,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;

        self.vertex_buffer = Some(vertex_buffer);
        self.vertex_buffer_memory = Some(vertex_buffer_memory);

        // Copy (vertex)

        copy_buffer(
            device,
            command_pool,
            graphics_queue,
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
        physical_device: &PhysicalDevice,
        command_pool: &CommandPool,
        graphics_queue: &Queue,
        meshes: &ObjDB<Mesh>,
    ) -> Result<()> {
        // Create (staging)

        let size = self.data_size_for_indexes() as u64;

        let (staging_buffer, staging_buffer_memory) = create_buffer(
            instance,
            device,
            physical_device,
            size,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_COHERENT | vk::MemoryPropertyFlags::HOST_VISIBLE,
        )?;

        // Copy (staging)

        let memory =
            device.map_memory(staging_buffer_memory, 0, size, vk::MemoryMapFlags::empty())?;

        let mut offset = 0usize;
        for mesh_handle in &self.mesh_handles {
            let mesh = meshes.get(*mesh_handle).unwrap();
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
            physical_device,
            size,
            vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::INDEX_BUFFER,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;

        self.index_buffer = Some(index_buffer);
        self.index_buffer_memory = Some(index_buffer_memory);

        // Copy (index)

        copy_buffer(
            device,
            command_pool,
            graphics_queue,
            staging_buffer,
            index_buffer,
            size,
        )?;

        // Cleanup

        device.destroy_buffer(staging_buffer, None);
        device.free_memory(staging_buffer_memory, None);

        Ok(())
    }

    pub fn cleanup(&mut self, device: &Device) {
        if self.vertex_buffer.is_some() {
            unsafe {
                device.destroy_buffer(self.vertex_buffer.unwrap(), None);
                device.free_memory(self.vertex_buffer_memory.unwrap(), None);
            }
            self.vertex_buffer = None;
            self.vertex_buffer_memory = None;
        }
        if self.index_buffer.is_some() {
            unsafe {
                device.destroy_buffer(self.index_buffer.unwrap(), None);
                device.free_memory(self.index_buffer_memory.unwrap(), None);
            }
            self.index_buffer = None;
            self.index_buffer_memory = None;
        }
    }

    pub fn prepare(
        &mut self,
        instance: &Instance,
        device: &Device,
        physical_device: &PhysicalDevice,
        command_pool: &CommandPool,
        graphics_queue: &Queue,
        meshes: &ObjDB<Mesh>,
    ) -> Result<()> {
        unsafe {
            self.cleanup(device);
            self.create_vertex_buffer(
                instance,
                device,
                physical_device,
                command_pool,
                graphics_queue,
                meshes,
            )?;
            self.create_index_buffer(
                instance,
                device,
                physical_device,
                command_pool,
                graphics_queue,
                meshes,
            )?;
        }
        Ok(())
    }

    pub fn is_prepared(&self) -> bool {
        self.vertex_buffer.is_some() && self.index_buffer.is_some()
    }
}
