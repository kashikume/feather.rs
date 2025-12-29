use super::meshbufferdata::MeshBufferData;
use super::object::Object;
use super::vertex::Vertex;
use std::mem::size_of;

#[derive(Default)]
pub struct Mesh {
    handle: usize,
    name: Option<String>,
    pub(crate) vertices: Vec<Vertex>,
    pub(crate) indices: Vec<u32>,
    pub(crate) mesh_buffer_data: Option<MeshBufferData>,
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
            indices,
            mesh_buffer_data: None,
        }
    }

    pub fn data_size_for_vertexes(&self) -> usize {
        size_of::<Vertex>() * self.vertices.len()
    }

    pub fn data_size_for_indexes(&self) -> usize {
        size_of::<u32>() * self.indices.len()
    }

    pub fn gen_num_vertexes(&self) -> usize {
        self.vertices.len()
    }

    pub fn gen_num_indexes(&self) -> usize {
        self.indices.len()
    }

    pub fn set_mesh_buffer_data(&mut self, buffer_data: MeshBufferData) {
        self.mesh_buffer_data = Some(buffer_data);
    }

    pub fn has_buffers_assigned(&self) -> bool {
        self.mesh_buffer_data.is_some()
    }
}
