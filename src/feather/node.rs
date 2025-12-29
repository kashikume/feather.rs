use std::collections::HashSet;

use cgmath::SquareMatrix;

use super::math::Mat4;
use super::object::Object;

pub struct Node {
    name: Option<String>,
    handle: usize,
    parent: Option<usize>,
    childreen: HashSet<usize>,
    transform: Mat4,
    global_transform: Option<Mat4>,
    transparent: bool,
    visible: bool,
    mesh: Option<usize>,
    material: Option<usize>,
}

impl Object for Node {
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

impl Node {
    fn internal_new(name: Option<String>, parent: Option<usize>, transparent: bool) -> Self {
        Self {
            name,
            handle: usize::MAX,
            parent: parent,
            childreen: HashSet::new(),
            transform: Mat4::identity(),
            global_transform: None,
            transparent,
            visible: true,
            mesh: None,
            material: None,
        }
    }

    pub fn new_root(name: Option<String>) -> Self {
        Self::internal_new(name, None, false)
    }

    pub fn new_with_transparency(name: Option<String>, parent: usize, transparent: bool) -> Self {
        Self::internal_new(name, Some(parent), transparent)
    }

    pub fn new(name: Option<String>, parent: usize) -> Self {
        Self::internal_new(name, Some(parent), false)
    }

    pub fn is_root(&self) -> bool {
        self.parent.is_none()
    }

    pub fn add_child(&mut self, child: usize) {
        self.childreen.insert(child);
    }

    pub fn remove_child(&mut self, child: usize) {
        self.childreen.remove(&child);
    }

    pub fn get_childreen(&self) -> &HashSet<usize> {
        &self.childreen
    }

    pub fn set_parent(&mut self, parent: Option<usize>) {
        self.parent = parent;
    }

    pub fn get_parent(&self) -> Option<usize> {
        self.parent
    }

    pub fn set_mesh(&mut self, mesh: usize) {
        self.mesh = Some(mesh);
    }

    pub fn get_mesh(&self) -> Option<usize> {
        self.mesh
    }

    pub fn remove_mesh(&mut self) {
        self.mesh = None;
    }
}
