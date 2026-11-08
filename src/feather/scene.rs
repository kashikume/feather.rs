use anyhow::Result;
use vulkanalia::Device;

use crate::feather::object::Object;

use super::{mesh::Mesh, meshbuffer::MeshBuffer, node::Node, objdb::ObjDB};

pub struct Scene {
    pub meshes: ObjDB<Mesh>,
    pub buffers: ObjDB<MeshBuffer>,
    pub nodes: ObjDB<Node>,
    needs_create_mesh_buffer: bool,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            meshes: ObjDB::new(),
            buffers: ObjDB::new(),
            nodes: ObjDB::new(),
            needs_create_mesh_buffer: false,
        }
    }

    pub fn add_mesh(&mut self, mesh: Mesh) -> usize {
        self.needs_create_mesh_buffer = true;
        self.meshes.add(mesh)
    }

    pub fn create_root_node(&mut self, name: Option<String>) -> usize {
        let node = Node::new_root(name);
        self.nodes.add(node)
    }

    pub fn create_node(&mut self, name: Option<String>, parent_handle: usize) -> Result<usize> {
        if self.nodes.get(parent_handle).is_none() {
            anyhow::bail!("Parent node {parent_handle} not found");
        }
        let node_handle = self.nodes.add(Node::new(name, parent_handle));
        self.nodes
            .get_mut(parent_handle)
            .unwrap()
            .add_child(node_handle);
        Ok(node_handle)
    }

    /// Detaches a node from its parent. The node stays in the scene, with no parent.
    pub fn disconnect_node(&mut self, node_handle: usize) -> Result<()> {
        let node = self
            .nodes
            .get_mut(node_handle)
            .ok_or_else(|| anyhow::anyhow!("Node {node_handle} not found"))?;
        let Some(parent) = node.get_parent() else {
            return Ok(());
        };
        node.set_parent(None);
        if let Some(parent_node) = self.nodes.get_mut(parent) {
            parent_node.remove_child(node_handle);
        }
        Ok(())
    }

    pub fn get_node(&self, handle: usize) -> Option<&Node> {
        self.nodes.get(handle)
    }

    pub fn get_node_mut(&mut self, handle: usize) -> Option<&mut Node> {
        self.nodes.get_mut(handle)
    }

    pub fn get_mesh(&self, handle: usize) -> Option<&Mesh> {
        self.meshes.get(handle)
    }

    pub fn node_set_mesh(&mut self, node_handle: usize, mesh_handle: usize) -> Result<()> {
        self.get_node_mut(node_handle)
            .ok_or(anyhow::anyhow!("Node not found"))?
            .set_mesh(mesh_handle);
        Ok(())
    }

    pub fn create_mesh_buffer(&mut self) -> usize {
        let mesh_buffer = MeshBuffer::new();
        self.buffers.add(mesh_buffer)
    }

    pub fn build_missing_mesh_buffers(&mut self) -> Result<()> {
        if !self.needs_create_mesh_buffer {
            return Ok(());
        }
        let mut to_build = Vec::new();
        for mesh in self.meshes.iter() {
            if !mesh.has_buffers_assigned() {
                to_build.push(mesh.get_handle());
            }
        }

        if to_build.is_empty() {
            self.needs_create_mesh_buffer = false;
            return Ok(());
        }

        let mesh_buffer_handle = self.create_mesh_buffer();
        let mesh_buffer = self.buffers.get_mut(mesh_buffer_handle).unwrap();

        for mesh_handle in to_build {
            let mesh = self.meshes.get_mut(mesh_handle).unwrap();
            mesh_buffer.add_mesh(mesh);
        }
        self.needs_create_mesh_buffer = false;
        Ok(())
    }

    pub fn destroy(&mut self, device: &Device) {
        for mesh_buffer in self.buffers.iter_mut() {
            mesh_buffer.cleanup(device);
        }
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feather::math::{Vec2, Vec3};
    use crate::feather::vertex::Vertex;

    /// Mesh with `num_vertices` dummy vertices and `num_indices` indices.
    fn test_mesh(num_vertices: usize, num_indices: usize) -> Mesh {
        let vertices = (0..num_vertices)
            .map(|i| {
                Vertex::new(
                    Vec3::new(i as f32, 0.0, 0.0),
                    Vec3::new(0.0, 0.0, 1.0),
                    Vec2::new(0.0, 0.0),
                )
            })
            .collect();
        let indices = (0..num_indices as u32).collect();
        Mesh::new(None, vertices, indices)
    }

    // --- nodes ---

    #[test]
    fn new_scene_is_empty() {
        let scene = Scene::new();
        assert_eq!(scene.meshes.iter().count(), 0);
        assert_eq!(scene.buffers.iter().count(), 0);
        assert_eq!(scene.nodes.iter().count(), 0);
    }

    #[test]
    fn default_scene_is_empty() {
        let scene = Scene::default();
        assert_eq!(scene.nodes.iter().count(), 0);
    }

    #[test]
    fn create_root_node() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(Some("root".to_string()));

        let node = scene.get_node(root).unwrap();
        assert!(node.is_root());
        assert_eq!(node.get_parent(), None);
        assert_eq!(node.get_handle(), root);
        assert_eq!(node.get_name(), Some("root".to_string()));
        assert!(node.get_childreen().is_empty());
        assert_eq!(node.get_mesh(), None);
    }

    #[test]
    fn create_node_links_parent_and_child() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(Some("root".to_string()));
        let child = scene.create_node(Some("child".to_string()), root).unwrap();

        assert_ne!(root, child);

        let child_node = scene.get_node(child).unwrap();
        assert!(!child_node.is_root());
        assert_eq!(child_node.get_parent(), Some(root));
        assert_eq!(child_node.get_handle(), child);

        let root_node = scene.get_node(root).unwrap();
        assert!(root_node.get_childreen().contains(&child));
        assert_eq!(root_node.get_childreen().len(), 1);
    }

    #[test]
    fn create_multiple_children() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let a = scene.create_node(Some("a".to_string()), root).unwrap();
        let b = scene.create_node(Some("b".to_string()), root).unwrap();
        let c = scene.create_node(None, root).unwrap();

        let children = scene.get_node(root).unwrap().get_childreen();
        assert_eq!(children.len(), 3);
        assert!(children.contains(&a));
        assert!(children.contains(&b));
        assert!(children.contains(&c));
    }

    #[test]
    fn create_nested_hierarchy() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let child = scene.create_node(None, root).unwrap();
        let grandchild = scene.create_node(None, child).unwrap();

        assert_eq!(
            scene.get_node(grandchild).unwrap().get_parent(),
            Some(child)
        );
        assert_eq!(scene.get_node(child).unwrap().get_parent(), Some(root));

        // Grandchild is a child of `child` only, not of the root.
        assert!(scene
            .get_node(child)
            .unwrap()
            .get_childreen()
            .contains(&grandchild));
        assert!(!scene
            .get_node(root)
            .unwrap()
            .get_childreen()
            .contains(&grandchild));
    }

    #[test]
    fn get_node_mut_modifies_node() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);

        scene.get_node_mut(root).unwrap().set_mesh(7);
        assert_eq!(scene.get_node(root).unwrap().get_mesh(), Some(7));
    }

    #[test]
    fn disconnect_node_removes_it_from_parent() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let a = scene.create_node(None, root).unwrap();
        let b = scene.create_node(None, root).unwrap();

        scene.disconnect_node(a).unwrap();

        let children = scene.get_node(root).unwrap().get_childreen();
        assert!(!children.contains(&a));
        assert!(children.contains(&b));
        // The node itself still exists in the scene, now without a parent.
        let node = scene.get_node(a).unwrap();
        assert_eq!(node.get_parent(), None);
        assert!(node.is_root());
    }

    #[test]
    fn disconnect_node_with_invalid_handle_fails() {
        let mut scene = Scene::new();
        assert!(scene.disconnect_node(42).is_err());
    }

    #[test]
    fn create_node_with_invalid_parent_fails() {
        let mut scene = Scene::new();
        assert!(scene.create_node(None, 42).is_err());
        // Nothing was added.
        assert_eq!(scene.nodes.iter().count(), 0);
    }

    #[test]
    fn nodes_with_same_name_do_not_overwrite_each_other() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let first = scene.create_node(Some("a".to_string()), root).unwrap();
        let grandchild = scene.create_node(None, first).unwrap();
        let second = scene.create_node(Some("a".to_string()), root).unwrap();

        assert_ne!(first, second);
        assert_eq!(scene.get_node(root).unwrap().get_childreen().len(), 2);
        // The first node keeps its own child.
        assert!(scene
            .get_node(first)
            .unwrap()
            .get_childreen()
            .contains(&grandchild));
        assert!(scene.get_node(second).unwrap().get_childreen().is_empty());
    }

    #[test]
    fn get_node_with_invalid_handle_returns_none() {
        let mut scene = Scene::new();
        assert!(scene.get_node(42).is_none());
        assert!(scene.get_node_mut(42).is_none());
        assert!(scene.get_mesh(42).is_none());
    }

    #[test]
    fn node_set_mesh_with_invalid_handle_fails() {
        let mut scene = Scene::new();
        assert!(scene.node_set_mesh(42, 0).is_err());
    }

    #[test]
    fn disconnect_root_node_is_noop() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let child = scene.create_node(None, root).unwrap();

        scene.disconnect_node(root).unwrap();

        assert!(scene
            .get_node(root)
            .unwrap()
            .get_childreen()
            .contains(&child));
    }

    // --- meshes ---

    #[test]
    fn add_mesh_and_get_mesh() {
        let mut scene = Scene::new();
        let m0 = scene.add_mesh(test_mesh(3, 3));
        let m1 = scene.add_mesh(test_mesh(4, 6));

        assert_ne!(m0, m1);
        assert_eq!(scene.get_mesh(m0).unwrap().gen_num_vertexes(), 3);
        assert_eq!(scene.get_mesh(m1).unwrap().gen_num_vertexes(), 4);
        assert_eq!(scene.get_mesh(m1).unwrap().get_handle(), m1);
    }

    #[test]
    fn node_set_mesh_assigns_mesh() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let mesh = scene.add_mesh(test_mesh(3, 3));

        scene.node_set_mesh(root, mesh).unwrap();

        assert_eq!(scene.get_node(root).unwrap().get_mesh(), Some(mesh));
    }

    #[test]
    fn node_set_mesh_on_removed_node_fails() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        scene.nodes.remove(root);

        assert!(scene.node_set_mesh(root, 0).is_err());
    }

    // --- mesh buffers ---

    #[test]
    fn create_mesh_buffer_returns_distinct_handles() {
        let mut scene = Scene::new();
        let b0 = scene.create_mesh_buffer();
        let b1 = scene.create_mesh_buffer();

        assert_ne!(b0, b1);
        assert_eq!(scene.buffers.iter().count(), 2);
        assert!(!scene.buffers.get(b0).unwrap().is_prepared());
    }

    #[test]
    fn build_missing_mesh_buffers_without_meshes_creates_nothing() {
        let mut scene = Scene::new();
        scene.build_missing_mesh_buffers().unwrap();
        assert_eq!(scene.buffers.iter().count(), 0);
    }

    #[test]
    fn build_missing_mesh_buffers_packs_meshes_into_one_buffer() {
        let mut scene = Scene::new();
        let m0 = scene.add_mesh(test_mesh(3, 6));
        let m1 = scene.add_mesh(test_mesh(4, 9));

        scene.build_missing_mesh_buffers().unwrap();

        assert_eq!(scene.buffers.iter().count(), 1);
        let buffer = scene.buffers.iter().next().unwrap();
        let buffer_handle = buffer.get_handle();
        assert_eq!(buffer.data_size_for_vertexes(), 7 * size_of::<Vertex>());
        assert_eq!(buffer.data_size_for_indexes(), 15 * size_of::<u32>());

        let d0 = scene
            .get_mesh(m0)
            .unwrap()
            .mesh_buffer_data
            .as_ref()
            .unwrap();
        assert_eq!(d0.buffer_handle, buffer_handle);
        assert_eq!(d0.vertex_begin_index, 0);
        assert_eq!(d0.vertex_size, 3);
        assert_eq!(d0.index_begin_index, 0);
        assert_eq!(d0.index_size, 6);

        // Second mesh starts right after the first one.
        let d1 = scene
            .get_mesh(m1)
            .unwrap()
            .mesh_buffer_data
            .as_ref()
            .unwrap();
        assert_eq!(d1.buffer_handle, buffer_handle);
        assert_eq!(d1.vertex_begin_index, 3);
        assert_eq!(d1.vertex_size, 4);
        assert_eq!(d1.index_begin_index, 6);
        assert_eq!(d1.index_size, 9);
    }

    #[test]
    fn build_missing_mesh_buffers_twice_does_not_create_another_buffer() {
        let mut scene = Scene::new();
        scene.add_mesh(test_mesh(3, 3));

        scene.build_missing_mesh_buffers().unwrap();
        scene.build_missing_mesh_buffers().unwrap();

        assert_eq!(scene.buffers.iter().count(), 1);
    }

    #[test]
    fn build_missing_mesh_buffers_only_builds_new_meshes() {
        let mut scene = Scene::new();
        let m0 = scene.add_mesh(test_mesh(3, 3));
        scene.build_missing_mesh_buffers().unwrap();
        let first_buffer = scene
            .get_mesh(m0)
            .unwrap()
            .mesh_buffer_data
            .as_ref()
            .unwrap()
            .buffer_handle;

        let m1 = scene.add_mesh(test_mesh(5, 6));
        scene.build_missing_mesh_buffers().unwrap();

        assert_eq!(scene.buffers.iter().count(), 2);

        // The first mesh keeps its original buffer.
        let d0 = scene
            .get_mesh(m0)
            .unwrap()
            .mesh_buffer_data
            .as_ref()
            .unwrap();
        assert_eq!(d0.buffer_handle, first_buffer);

        // The new mesh goes into a new buffer, starting at offset 0.
        let d1 = scene
            .get_mesh(m1)
            .unwrap()
            .mesh_buffer_data
            .as_ref()
            .unwrap();
        assert_ne!(d1.buffer_handle, first_buffer);
        assert_eq!(d1.vertex_begin_index, 0);
        assert_eq!(d1.index_begin_index, 0);

        let new_buffer = scene.buffers.get(d1.buffer_handle).unwrap();
        assert_eq!(new_buffer.data_size_for_vertexes(), 5 * size_of::<Vertex>());
        assert_eq!(new_buffer.data_size_for_indexes(), 6 * size_of::<u32>());
    }
}
