use std::collections::HashSet;

use cgmath::SquareMatrix;

use super::math::Mat4;
use super::object::Object;
use super::uniforms::Uniforms;

pub struct Node {
    name: Option<String>,
    handle: usize,
    parent: Option<usize>,
    children: HashSet<usize>,
    transform: Mat4,
    global_transform: Option<Mat4>,
    transparent: bool,
    visible: bool,
    mesh: Option<usize>,
    material: Option<usize>,
    /// Uniforms set on this node.
    uniforms: Uniforms,
    /// Uniforms inherited from the parent, overridden by `uniforms`. Computed by
    /// `Scene::update_dynamic_uniforms`.
    dynamic_uniforms: Uniforms,
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
            children: HashSet::new(),
            transform: Mat4::identity(),
            global_transform: None,
            transparent,
            visible: true,
            mesh: None,
            material: None,
            uniforms: Uniforms::new(),
            dynamic_uniforms: Uniforms::new(),
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
        self.children.insert(child);
    }

    pub fn remove_child(&mut self, child: usize) {
        self.children.remove(&child);
    }

    pub fn get_children(&self) -> &HashSet<usize> {
        &self.children
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

    pub fn uniforms(&self) -> &Uniforms {
        &self.uniforms
    }

    pub fn uniforms_mut(&mut self) -> &mut Uniforms {
        &mut self.uniforms
    }

    /// Uniforms inherited from the parent, overridden by this node's own uniforms.
    /// Valid after `Scene::update_dynamic_uniforms`.
    pub fn dynamic_uniforms(&self) -> &Uniforms {
        &self.dynamic_uniforms
    }

    /// Recomputes `dynamic_uniforms`: starts from `inherited` (the parent's dynamic uniforms)
    /// and applies this node's own uniforms on top, replacing ones with the same name.
    pub(crate) fn update_dynamic_uniforms(&mut self, inherited: &Uniforms) {
        let mut dynamic = inherited.clone();
        for uniform in &self.uniforms {
            dynamic.set(uniform.name.clone(), uniform.value);
        }
        self.dynamic_uniforms = dynamic;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_root_has_no_parent() {
        let node = Node::new_root(Some("root".to_string()));
        assert!(node.is_root());
        assert_eq!(node.get_parent(), None);
        assert_eq!(node.get_name(), Some("root".to_string()));
    }

    #[test]
    fn new_has_parent() {
        let node = Node::new(None, 3);
        assert!(!node.is_root());
        assert_eq!(node.get_parent(), Some(3));
        assert_eq!(node.get_name(), None);
        assert!(!node.transparent);
    }

    #[test]
    fn new_with_transparency() {
        let transparent = Node::new_with_transparency(None, 1, true);
        assert_eq!(transparent.get_parent(), Some(1));
        assert!(transparent.transparent);

        let opaque = Node::new_with_transparency(None, 1, false);
        assert!(!opaque.transparent);
    }

    #[test]
    fn new_node_defaults() {
        let node = Node::new_root(None);
        assert_eq!(node.get_handle(), usize::MAX);
        assert!(node.get_children().is_empty());
        assert_eq!(node.get_mesh(), None);
        assert_eq!(node.material, None);
        assert_eq!(node.transform, Mat4::identity());
        assert_eq!(node.global_transform, None);
        assert!(node.visible);
        assert!(!node.transparent);
    }

    #[test]
    fn set_handle() {
        let mut node = Node::new_root(None);
        node.set_handle(5);
        assert_eq!(node.get_handle(), 5);
    }

    #[test]
    fn add_and_remove_children() {
        let mut node = Node::new_root(None);
        node.add_child(1);
        node.add_child(2);
        assert_eq!(node.get_children().len(), 2);
        assert!(node.get_children().contains(&1));
        assert!(node.get_children().contains(&2));

        node.remove_child(1);
        assert_eq!(node.get_children().len(), 1);
        assert!(!node.get_children().contains(&1));
        assert!(node.get_children().contains(&2));
    }

    #[test]
    fn add_same_child_twice_keeps_one() {
        let mut node = Node::new_root(None);
        node.add_child(1);
        node.add_child(1);
        assert_eq!(node.get_children().len(), 1);
    }

    #[test]
    fn remove_missing_child_is_noop() {
        let mut node = Node::new_root(None);
        node.add_child(1);
        node.remove_child(2);
        assert_eq!(node.get_children().len(), 1);
    }

    #[test]
    fn set_parent_changes_root_status() {
        let mut node = Node::new_root(None);
        node.set_parent(Some(4));
        assert!(!node.is_root());
        assert_eq!(node.get_parent(), Some(4));

        node.set_parent(None);
        assert!(node.is_root());
        assert_eq!(node.get_parent(), None);
    }

    #[test]
    fn new_node_has_no_uniforms() {
        let node = Node::new_root(None);
        assert!(node.uniforms().is_empty());
    }

    #[test]
    fn node_keeps_its_own_uniforms() {
        let mut a = Node::new_root(None);
        let b = Node::new_root(None);
        a.uniforms_mut().set("tint", 0.5f32);

        assert_eq!(
            a.uniforms().get("tint"),
            Some(&crate::feather::uniform::UniformValue::Float(0.5))
        );
        assert!(b.uniforms().is_empty());
    }

    #[test]
    fn dynamic_uniforms_merge_inherited_and_own() {
        use crate::feather::uniform::UniformValue;

        let mut inherited = Uniforms::new();
        inherited.set("a", 1u32).set("b", 2u32);

        let mut node = Node::new_root(None);
        node.uniforms_mut().set("b", 20u32).set("c", 30u32);
        node.update_dynamic_uniforms(&inherited);

        let dynamic = node.dynamic_uniforms();
        assert_eq!(dynamic.get("a"), Some(&UniformValue::UInt(1)));
        assert_eq!(dynamic.get("b"), Some(&UniformValue::UInt(20))); // own wins
        assert_eq!(dynamic.get("c"), Some(&UniformValue::UInt(30)));
        // Inherited first (overrides keep their place), then new own ones.
        let names: Vec<_> = dynamic.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, ["a", "b", "c"]);
        // Own uniforms are unchanged.
        assert_eq!(node.uniforms().len(), 2);
    }

    #[test]
    fn dynamic_uniforms_are_replaced_on_update() {
        let mut first = Uniforms::new();
        first.set("old", 1u32);

        let mut node = Node::new_root(None);
        node.update_dynamic_uniforms(&first);
        node.update_dynamic_uniforms(&Uniforms::new());

        assert!(node.dynamic_uniforms().is_empty());
    }

    #[test]
    fn set_replace_and_remove_mesh() {
        let mut node = Node::new_root(None);
        node.set_mesh(2);
        assert_eq!(node.get_mesh(), Some(2));

        node.set_mesh(5);
        assert_eq!(node.get_mesh(), Some(5));

        node.remove_mesh();
        assert_eq!(node.get_mesh(), None);
    }
}
