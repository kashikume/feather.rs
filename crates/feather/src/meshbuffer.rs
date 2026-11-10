use std::mem::size_of;
use std::sync::atomic::{AtomicU64, Ordering};

use super::mesh::Mesh;
use super::meshbufferdata::MeshBufferData;
use super::object::Object;
use super::vertex::Vertex;

/// Source of `MeshBuffer::get_id` values.
static NEXT_MESH_BUFFER_ID: AtomicU64 = AtomicU64::new(0);

/// A place in a mesh buffer: the mesh stored there (`None` once removed) and its size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeshBufferSlot {
    pub mesh: Option<usize>,
    pub num_vertexes: usize,
    pub num_indexes: usize,
}

/// Meshes packed one after another into a shared vertex buffer and index buffer. This is the
/// layout only; the renderer uploads it to the GPU (`GpuMeshBuffer`).
///
/// A removed mesh leaves its slot behind, so the meshes after it keep their places; the buffer
/// is removed from the scene with its last mesh (`Scene::remove_mesh`).
pub struct MeshBuffer {
    handle: usize,
    id: u64,
    num_vertexes: usize,
    num_indexes: usize,
    slots: Vec<MeshBufferSlot>,
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

impl Default for MeshBuffer {
    fn default() -> Self {
        Self {
            handle: usize::MAX,
            id: NEXT_MESH_BUFFER_ID.fetch_add(1, Ordering::Relaxed),
            num_vertexes: 0,
            num_indexes: 0,
            slots: Vec::new(),
        }
    }
}

impl MeshBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Identifies the buffer: no two buffers created by the process have the same id, while
    /// handles of removed buffers are reused. The renderer uses it to tell a new buffer from
    /// a removed one that had the same handle.
    pub fn get_id(&self) -> u64 {
        self.id
    }

    /// Appends `mesh` to the buffer and tells the mesh where it is.
    pub fn add_mesh(&mut self, mesh: &mut Mesh) {
        self.slots.push(MeshBufferSlot {
            mesh: Some(mesh.get_handle()),
            num_vertexes: mesh.gen_num_vertexes(),
            num_indexes: mesh.gen_num_indexes(),
        });
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

    /// Forgets mesh `mesh`; its slot stays, empty. Returns whether the buffer has no meshes
    /// left.
    pub(crate) fn remove_mesh(&mut self, mesh: usize) -> bool {
        for slot in &mut self.slots {
            if slot.mesh == Some(mesh) {
                slot.mesh = None;
            }
        }
        self.is_empty()
    }

    /// Whether the buffer holds no meshes (none added, or all removed).
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(|s| s.mesh.is_none())
    }

    /// The slots of the buffer, in order: where each mesh is, or was.
    pub fn get_slots(&self) -> &[MeshBufferSlot] {
        &self.slots
    }

    /// Handles of the meshes in the buffer, in order.
    pub fn get_mesh_handles(&self) -> Vec<usize> {
        self.slots.iter().filter_map(|s| s.mesh).collect()
    }

    pub fn data_size_for_vertexes(&self) -> usize {
        self.num_vertexes * size_of::<Vertex>()
    }

    pub fn data_size_for_indexes(&self) -> usize {
        self.num_indexes * size_of::<u32>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{Vec2, Vec3};

    fn mesh(handle: usize, vertexes: usize, indexes: usize) -> Mesh {
        let vertex = Vertex::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec2::new(0.0, 0.0),
        );
        let mut mesh = Mesh::new(None, vec![vertex; vertexes], vec![0; indexes]);
        mesh.set_handle(handle);
        mesh
    }

    #[test]
    fn ids_are_unique() {
        assert_ne!(MeshBuffer::new().get_id(), MeshBuffer::new().get_id());
    }

    #[test]
    fn removed_meshes_leave_their_slots() {
        let mut buffer = MeshBuffer::new();
        assert!(buffer.is_empty());
        buffer.add_mesh(&mut mesh(4, 3, 6));
        buffer.add_mesh(&mut mesh(7, 2, 3));
        assert!(!buffer.is_empty());

        assert!(!buffer.remove_mesh(4));
        assert_eq!(buffer.get_mesh_handles(), vec![7]);
        assert_eq!(
            buffer.get_slots()[0],
            MeshBufferSlot {
                mesh: None,
                num_vertexes: 3,
                num_indexes: 6
            }
        );
        assert_eq!(buffer.data_size_for_vertexes(), 5 * size_of::<Vertex>());

        assert!(buffer.remove_mesh(7));
        assert!(buffer.is_empty());
    }
}
