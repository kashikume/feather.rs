use cgmath::SquareMatrix;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

use super::material::Material;
use super::math::Mat4;
use super::mesh::Mesh;

pub struct Node {
    parent: Option<Weak<Node>>,
    childreen: RefCell<Vec<Rc<Node>>>,
    transform: Mat4,
    global_transform: Option<Mat4>,
    transparent: bool,
    visible: bool,
    mesh: Option<Rc<Mesh>>,
    material: Option<Rc<Material>>,
}

impl Node {
    fn internal_new(parent: Option<Rc<Node>>, transparent: bool) -> Rc<Self> {
        let node = Rc::new(Self {
            parent: match parent {
                Some(ref parent) => Some(Rc::downgrade(&parent)),
                None => None,
            },
            childreen: RefCell::new(Vec::new()),
            transform: Mat4::identity(),
            global_transform: None,
            transparent,
            visible: true,
            mesh: None,
            material: None,
        });
        if parent.is_some() {
            parent.unwrap().childreen.borrow_mut().push(node.clone());
        }
        node
    }

    pub fn new_root() -> Rc<Self> {
        Self::internal_new(None, false)
    }

    pub fn new_with_transparency(parent: &Rc<Self>, transparent: bool) -> Rc<Self> {
        Self::internal_new(Some(parent.clone()), transparent)
    }

    pub fn new(parent: &Rc<Self>) -> Rc<Self> {
        Self::internal_new(Some(parent.clone()), false)
    }

    pub fn is_root(&self) -> bool {
        self.parent.is_none()
    }
}
