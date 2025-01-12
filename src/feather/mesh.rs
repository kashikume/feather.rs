use super::vertex::Vertex;
use std::mem::size_of;

#[derive(Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Mesh {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn data_size_for_vertexes(&self) -> usize {
        size_of::<Vertex>() * self.vertices.len()
    }

    pub fn data_size_for_indexes(&self) -> usize {
        size_of::<u32>() * self.indices.len()
    }
}
