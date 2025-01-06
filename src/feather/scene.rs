use std::rc::Rc;

use super::node::Node;

pub struct Scene {
    pub root: Rc<Node>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            root: Node::new_root(),
        }
    }
}
