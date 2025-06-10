use vulkanalia::vk;

use super::object::Object;

#[derive(Default)]
pub struct MeshBuffer {
    handle: usize,
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