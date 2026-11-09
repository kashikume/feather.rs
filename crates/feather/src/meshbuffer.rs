use std::mem::size_of;

use super::mesh::Mesh;
use super::meshbufferdata::MeshBufferData;
use super::object::Object;
use super::vertex::Vertex;

/// Meshes packed one after another into a shared vertex buffer and index buffer. This is the
/// layout only; the renderer uploads it to the GPU (`GpuMeshBuffer`).
#[derive(Default)]
pub struct MeshBuffer {
    handle: usize,
    num_vertexes: usize,
    num_indexes: usize,
    mesh_handles: Vec<usize>,
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
        Self::default()
    }

    /// Appends `mesh` to the buffer and tells the mesh where it is.
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

    /// Handles of the meshes in the buffer, in order.
    pub fn get_mesh_handles(&self) -> &[usize] {
        &self.mesh_handles
    }

    pub fn data_size_for_vertexes(&self) -> usize {
        self.num_vertexes * size_of::<Vertex>()
    }

    pub fn data_size_for_indexes(&self) -> usize {
        self.num_indexes * size_of::<u32>()
    }
}
