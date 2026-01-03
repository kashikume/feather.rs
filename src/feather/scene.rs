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
