use anyhow::Result;

use crate::feather::object::Object;

use super::{mesh::Mesh, meshbuffer::MeshBuffer, node::Node, objdb::ObjDB};

pub struct Scene {
    pub meshes: ObjDB<Mesh>,
    pub buffers: ObjDB<MeshBuffer>,
    pub nodes: ObjDB<Node>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            meshes: ObjDB::new(),
            buffers: ObjDB::new(),
            nodes: ObjDB::new(),
        }
    }

    pub fn add_mesh(&mut self, mesh: Mesh) -> usize {
        self.meshes.add(mesh)
    }

    pub fn create_root_node(&mut self, name: Option<String>) -> usize {
        let node = Node::new_root(name);
        self.nodes.add(node)
    }

    pub fn create_node(&mut self, name: Option<String>, parent_handle: usize) -> usize {
        let node = Node::new(name, parent_handle);
        let node_handle = self.nodes.add(node);
        let parent_node = self.nodes.get_mut(parent_handle).unwrap();
        parent_node.add_child(node_handle);
        node_handle
    }

    pub fn disconnect_node(&mut self, node_handle: usize) {
        let node = self.nodes.get_mut(node_handle).unwrap();
        if let Some(parent) = node.get_parent() {
            let parent_node = self.nodes.get_mut(parent).unwrap();
            parent_node.remove_child(node_handle);
        }
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
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}
