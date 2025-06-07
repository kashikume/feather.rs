use std::{collections::HashMap, rc::Rc};

use super::{idgen::IdGen, mesh::Mesh, node::Node};

pub struct Scene {
    pub id_gen: IdGen,
    pub meshes: HashMap<u64, Rc<Mesh>>,
    pub root: Rc<Node>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            id_gen: IdGen::default(),
            root: Node::new_root(),
            meshes: HashMap::new(),
        }
    }

    pub fn add_mesh(&mut self, mesh: Rc<Mesh>) {
        self.meshes.insert(mesh.id, mesh);
    }

    pub fn get_mesh(&self, id: u64) -> Option<Rc<Mesh>> {
        self.meshes.get(&id).cloned()
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}
