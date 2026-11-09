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
    /// Transform relative to the parent node.
    transform: Mat4,
    /// Transform relative to the scene (parent's global transform × `transform`).
    /// Computed by `Scene::update_nodes`.
    global_transform: Option<Mat4>,
    transparent: bool,
    visible: bool,
    mesh: Option<usize>,
    material: Option<usize>,
    /// Uniforms set on this node.
    uniforms: Uniforms,
    /// Uniforms inherited from ancestor nodes (their own uniforms, not their materials').
    /// Computed by `Scene::update_nodes`.
    inherited_uniforms: Uniforms,
    /// Final uniforms of this node: material, then `inherited_uniforms`, then `uniforms`,
    /// each overriding the previous. Computed by `Scene::update_nodes`.
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
            inherited_uniforms: Uniforms::new(),
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

    /// Sets the transform relative to the parent node.
    pub fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
    }

    pub fn get_transform(&self) -> Mat4 {
        self.transform
    }

    /// Transform relative to the scene. `None` until `Scene::update_nodes` has run.
    pub fn get_global_transform(&self) -> Option<Mat4> {
        self.global_transform
    }

    /// Sets whether the node and its descendants are drawn.
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
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

    /// Sets the handle of the node's material (in `Scene::materials`).
    pub fn set_material(&mut self, material: usize) {
        self.material = Some(material);
    }

    pub fn get_material(&self) -> Option<usize> {
        self.material
    }

    pub fn remove_material(&mut self) {
        self.material = None;
    }

    pub fn uniforms(&self) -> &Uniforms {
        &self.uniforms
    }

    pub fn uniforms_mut(&mut self) -> &mut Uniforms {
        &mut self.uniforms
    }

    /// Final uniforms of this node: its material's uniforms, overridden by uniforms inherited
    /// from ancestor nodes, overridden by the node's own uniforms.
    /// Valid after `Scene::update_nodes`.
    pub fn dynamic_uniforms(&self) -> &Uniforms {
        &self.dynamic_uniforms
    }

    /// Uniforms inherited from ancestor nodes. Materials of ancestors are not included.
    /// Valid after `Scene::update_nodes`.
    pub fn inherited_uniforms(&self) -> &Uniforms {
        &self.inherited_uniforms
    }

    /// Recomputes `inherited_uniforms` and `dynamic_uniforms`.
    ///
    /// `inherited` comes from the parent (`uniforms_for_children` of the parent), `material`
    /// is the uniforms of this node's material, if it has one. Later layers override earlier
    /// ones with the same name: material, then inherited, then own.
    pub(crate) fn update_dynamic_uniforms(
        &mut self,
        inherited: &Uniforms,
        material: Option<&Uniforms>,
    ) {
        self.inherited_uniforms = inherited.clone();

        let mut dynamic = material.cloned().unwrap_or_default();
        for uniform in inherited.iter().chain(self.uniforms.iter()) {
            dynamic.set(uniform.name.clone(), uniform.value);
        }
        self.dynamic_uniforms = dynamic;
    }

    /// Recomputes the global transform from the parent's global transform (`None` for a root).
    pub(crate) fn update_global_transform(&mut self, parent: Option<&Mat4>) {
        self.global_transform = Some(match parent {
            Some(parent) => parent * self.transform,
            None => self.transform,
        });
    }

    /// Uniforms this node passes to its children: inherited ones overridden by its own.
    /// The node's material is not passed down; it applies to this node only.
    pub(crate) fn uniforms_for_children(&self) -> Uniforms {
        let mut result = self.inherited_uniforms.clone();
        for uniform in &self.uniforms {
            result.set(uniform.name.clone(), uniform.value);
        }
        result
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
        assert_eq!(node.get_material(), None);
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
            Some(&crate::uniform::UniformValue::Float(0.5))
        );
        assert!(b.uniforms().is_empty());
    }

    #[test]
    fn dynamic_uniforms_merge_inherited_and_own() {
        use crate::uniform::UniformValue;

        let mut inherited = Uniforms::new();
        inherited.set("a", 1u32).set("b", 2u32);

        let mut node = Node::new_root(None);
        node.uniforms_mut().set("b", 20u32).set("c", 30u32);
        node.update_dynamic_uniforms(&inherited, None);

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
        node.update_dynamic_uniforms(&first, None);
        node.update_dynamic_uniforms(&Uniforms::new(), None);

        assert!(node.dynamic_uniforms().is_empty());
        assert!(node.inherited_uniforms().is_empty());
    }

    #[test]
    fn dynamic_uniforms_layer_material_inherited_and_own() {
        use crate::uniform::UniformValue;

        let mut material = Uniforms::new();
        material.set("m", 1u32).set("x", 1u32).set("y", 1u32);
        let mut inherited = Uniforms::new();
        inherited.set("x", 2u32).set("y", 2u32);

        let mut node = Node::new_root(None);
        node.uniforms_mut().set("y", 3u32);
        node.update_dynamic_uniforms(&inherited, Some(&material));

        let dynamic = node.dynamic_uniforms();
        assert_eq!(dynamic.get("m"), Some(&UniformValue::UInt(1))); // material only
        assert_eq!(dynamic.get("x"), Some(&UniformValue::UInt(2))); // inherited beats material
        assert_eq!(dynamic.get("y"), Some(&UniformValue::UInt(3))); // own beats both
        assert_eq!(node.inherited_uniforms(), &inherited);
    }

    #[test]
    fn uniforms_for_children_exclude_material() {
        use crate::uniform::UniformValue;

        let mut material = Uniforms::new();
        material.set("m", 1u32);
        let mut inherited = Uniforms::new();
        inherited.set("a", 1u32).set("b", 1u32);

        let mut node = Node::new_root(None);
        node.uniforms_mut().set("b", 2u32);
        node.update_dynamic_uniforms(&inherited, Some(&material));

        let for_children = node.uniforms_for_children();
        assert_eq!(for_children.get("m"), None);
        assert_eq!(for_children.get("a"), Some(&UniformValue::UInt(1)));
        assert_eq!(for_children.get("b"), Some(&UniformValue::UInt(2)));
    }

    #[test]
    fn set_replace_and_remove_material() {
        let mut node = Node::new_root(None);
        node.set_material(2);
        assert_eq!(node.get_material(), Some(2));

        node.set_material(5);
        assert_eq!(node.get_material(), Some(5));

        node.remove_material();
        assert_eq!(node.get_material(), None);
    }

    #[test]
    fn set_transform() {
        use cgmath::{vec3, Matrix4};

        let mut node = Node::new_root(None);
        let t = Matrix4::from_translation(vec3(1.0, 2.0, 3.0));
        node.set_transform(t);
        assert_eq!(node.get_transform(), t);
        assert_eq!(node.get_global_transform(), None);
    }

    #[test]
    fn global_transform_of_root_is_its_transform() {
        use cgmath::{vec3, Matrix4};

        let mut node = Node::new_root(None);
        let t = Matrix4::from_translation(vec3(1.0, 0.0, 0.0));
        node.set_transform(t);
        node.update_global_transform(None);
        assert_eq!(node.get_global_transform(), Some(t));
    }

    #[test]
    fn global_transform_applies_parent_first() {
        use cgmath::{vec3, vec4, Deg, Matrix4};

        // Parent moves by +X; child rotates 90° around Z. A point at +X in the child ends up
        // at (1, 1): rotated by the child, then moved by the parent.
        let parent = Matrix4::from_translation(vec3(1.0, 0.0, 0.0));
        let mut node = Node::new(None, 0);
        node.set_transform(Matrix4::from_angle_z(Deg(90.0)));
        node.update_global_transform(Some(&parent));

        let p = node.get_global_transform().unwrap() * vec4(1.0, 0.0, 0.0, 1.0);
        assert!((p.x - 1.0).abs() < 1e-6 && (p.y - 1.0).abs() < 1e-6, "{p:?}");
    }

    #[test]
    fn visibility() {
        let mut node = Node::new_root(None);
        assert!(node.is_visible());
        node.set_visible(false);
        assert!(!node.is_visible());
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
