use super::idgen::IdGen;
use super::vertex::Vertex;
use std::mem::size_of;

#[derive(Default)]
pub struct Mesh {
    pub id: u64,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Mesh {
    pub fn new(id_gen: &mut IdGen, vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        Self {
            id: id_gen.next(),
            vertices,
            indices,
        }
    }

    pub fn data_size_for_vertexes(&self) -> usize {
        size_of::<Vertex>() * self.vertices.len()
    }

    pub fn data_size_for_indexes(&self) -> usize {
        size_of::<u32>() * self.indices.len()
    }
}
