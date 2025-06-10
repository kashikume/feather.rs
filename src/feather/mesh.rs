use super::bufferdata::BufferData;
use super::object::Object;
use super::vertex::Vertex;
use std::mem::size_of;

#[derive(Default)]
pub struct Mesh {
    handle: usize,
    name: Option<String>,
    pub(crate) vertices: Vec<Vertex>,
    pub(crate) vertex_buffer_data: Option<BufferData>,
    pub(crate) indices: Vec<u32>,
    pub(crate) index_buffer_data: Option<BufferData>,
}

impl Object for Mesh {
    fn get_name(&self) -> Option<String> {
        self.name.clone()
    }

    fn set_handle(&mut self, handle: usize) {
        self.handle = handle;
    }

    fn get_handle(&self) -> usize {
        self.handle
    }
}  

impl Mesh {
    pub fn new(name: Option<String>, vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        Self {
            handle: usize::MAX,
            name: None,
            vertices,
            vertex_buffer_data: None,
            indices,
            index_buffer_data: None,
        }
    }

    pub fn data_size_for_vertexes(&self) -> usize {
        size_of::<Vertex>() * self.vertices.len()
    }

    pub fn data_size_for_indexes(&self) -> usize {
        size_of::<u32>() * self.indices.len()
    }

    pub fn set_vertex_buffer_data(&mut self, buffer_data: BufferData) {
        self.vertex_buffer_data = Some(buffer_data);
    }

    pub fn set_index_buffer_data(&mut self, buffer_data: BufferData) {
        self.index_buffer_data = Some(buffer_data);
    }

    pub(crate) fn reset_buffer_data(&mut self) {
        self.vertex_buffer_data = None;
        self.index_buffer_data = None;
    }
}
