use super::bounds::Aabb;
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
    /// Box around the vertices, in the mesh's own space; `None` without vertices.
    bounds: Option<Aabb>,
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
        let bounds = Aabb::from_points(vertices.iter().map(|v| v.pos));
        Self {
            handle: usize::MAX,
            name,
            vertices,
            indices,
            mesh_buffer_data: None,
            bounds,
        }
    }

    pub fn get_vertices(&self) -> &[Vertex] {
        &self.vertices
    }

    /// The box around the mesh's vertices, in its own space (for frustum culling, see
    /// `bounds`). `None` for a mesh without vertices.
    pub fn get_bounds(&self) -> Option<&Aabb> {
        self.bounds.as_ref()
    }

    pub fn get_indices(&self) -> &[u32] {
        &self.indices
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_keeps_name() {
        let mesh = Mesh::new(Some("cube".to_string()), Vec::new(), Vec::new());
        assert_eq!(mesh.get_name(), Some("cube".to_string()));

        let unnamed = Mesh::new(None, Vec::new(), Vec::new());
        assert_eq!(unnamed.get_name(), None);
    }

    #[test]
    fn bounds_hold_the_vertices() {
        use crate::math::{Vec2, Vec3};
        let vertex = |x, y, z| Vertex::new(Vec3::new(x, y, z), Vec3::unit_z(), Vec2::new(0.0, 0.0));
        let mesh = Mesh::new(
            None,
            vec![
                vertex(1.0, 2.0, 3.0),
                vertex(-1.0, 0.0, 5.0),
                vertex(0.0, -2.0, 4.0),
            ],
            vec![0, 1, 2],
        );
        let bounds = mesh.get_bounds().unwrap();
        assert_eq!(bounds.min, Vec3::new(-1.0, -2.0, 3.0));
        assert_eq!(bounds.max, Vec3::new(1.0, 2.0, 5.0));
    }

    #[test]
    fn mesh_without_vertices_has_no_bounds() {
        assert!(Mesh::new(None, Vec::new(), Vec::new())
            .get_bounds()
            .is_none());
    }
}
