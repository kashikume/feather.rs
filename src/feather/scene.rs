use anyhow::Result;
use vulkanalia::Device;

use crate::feather::{
    assetsprovider::AssetsProvider, atlas::Atlas, material::Material, object::Object, shader::Shader, texture::Texture,
};

use super::{mesh::Mesh, meshbuffer::MeshBuffer, node::Node, objdb::ObjDB, uniforms::Uniforms};

pub struct Scene {
    pub meshes: ObjDB<Mesh>,
    pub buffers: ObjDB<MeshBuffer>,
    pub nodes: ObjDB<Node>,
    pub atlases: ObjDB<Atlas>,
    pub textures: ObjDB<Texture>,
    pub shaders: ObjDB<Shader>,
    pub materials: ObjDB<Material>,
    needs_create_mesh_buffer: bool,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            meshes: ObjDB::new(),
            buffers: ObjDB::new(),
            nodes: ObjDB::new(),
            atlases: ObjDB::new(),
            textures: ObjDB::new(),
            shaders: ObjDB::new(),
            materials: ObjDB::new(),
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

    /// Assigns a material to a node. Fails if the node or the material does not exist.
    pub fn node_set_material(&mut self, node_handle: usize, material_handle: usize) -> Result<()> {
        if self.materials.get(material_handle).is_none() {
            anyhow::bail!("Material {material_handle} not found");
        }
        self.get_node_mut(node_handle)
            .ok_or_else(|| anyhow::anyhow!("Node {node_handle} not found"))?
            .set_material(material_handle);
        Ok(())
    }

    // --- shaders and materials ---

    pub fn add_shader(&mut self, shader: Shader) -> usize {
        self.shaders.add(shader)
    }

    /// Creates a shader from vertex and fragment SPIR-V and adds it to the scene.
    /// See `Shader::new` for the uniform name parameters.
    pub fn create_shader(
        &mut self,
        name: Option<String>,
        vertex: &[u8],
        fragment: &[u8],
        uniform_model_name: Option<String>,
        uniform_view_name: Option<String>,
        uniform_proj_name: Option<String>,
    ) -> Result<usize> {
        let shader = Shader::new(
            name,
            vertex,
            fragment,
            uniform_model_name,
            uniform_view_name,
            uniform_proj_name,
        )?;
        Ok(self.add_shader(shader))
    }

    pub fn get_shader(&self, handle: usize) -> Option<&Shader> {
        self.shaders.get(handle)
    }

    pub fn get_shader_mut(&mut self, handle: usize) -> Option<&mut Shader> {
        self.shaders.get_mut(handle)
    }

    /// Creates a material using the given shader. Fails if the shader does not exist.
    pub fn create_material(&mut self, name: Option<String>, shader_handle: usize) -> Result<usize> {
        if self.shaders.get(shader_handle).is_none() {
            anyhow::bail!("Shader {shader_handle} not found");
        }
        Ok(self.materials.add(Material::new(name, shader_handle)))
    }

    pub fn get_material(&self, handle: usize) -> Option<&Material> {
        self.materials.get(handle)
    }

    pub fn get_material_mut(&mut self, handle: usize) -> Option<&mut Material> {
        self.materials.get_mut(handle)
    }

    /// Changes a material's shader. Fails if the material or the shader does not exist.
    pub fn material_set_shader(
        &mut self,
        material_handle: usize,
        shader_handle: usize,
    ) -> Result<()> {
        if self.shaders.get(shader_handle).is_none() {
            anyhow::bail!("Shader {shader_handle} not found");
        }
        self.materials
            .get_mut(material_handle)
            .ok_or_else(|| anyhow::anyhow!("Material {material_handle} not found"))?
            .set_shader(shader_handle);
        Ok(())
    }

    /// Recomputes dynamic uniforms of every node, starting from the root nodes.
    pub fn update_dynamic_uniforms(&mut self) {
        let roots: Vec<usize> = self
            .nodes
            .iter()
            .filter(|n| n.is_root())
            .map(|n| n.get_handle())
            .collect();
        for root in roots {
            self.update_subtree_dynamic_uniforms(root, &Uniforms::new());
        }
    }

    /// Recomputes dynamic uniforms of a node and all its descendants. Use it after changing
    /// uniforms of one node or of its material; the node's parent must already be up to date.
    pub fn update_node_dynamic_uniforms(&mut self, node_handle: usize) -> Result<()> {
        let node = self
            .nodes
            .get(node_handle)
            .ok_or_else(|| anyhow::anyhow!("Node {node_handle} not found"))?;
        let inherited = node
            .get_parent()
            .and_then(|p| self.nodes.get(p))
            .map(|p| p.uniforms_for_children())
            .unwrap_or_default();
        self.update_subtree_dynamic_uniforms(node_handle, &inherited);
        Ok(())
    }

    fn update_subtree_dynamic_uniforms(&mut self, node_handle: usize, inherited: &Uniforms) {
        // Depth-first, without recursion: (node, uniforms inherited from its parent).
        let mut stack = vec![(node_handle, inherited.clone())];
        while let Some((handle, inherited)) = stack.pop() {
            let Some(node) = self.nodes.get_mut(handle) else {
                continue;
            };
            let material = node
                .get_material()
                .and_then(|m| self.materials.get(m))
                .map(|m| m.uniforms());
            node.update_dynamic_uniforms(&inherited, material);
            let for_children = node.uniforms_for_children();
            for &child in node.get_children() {
                stack.push((child, for_children.clone()));
            }
        }
    }

    pub fn add_atlas(&mut self, atlas: Atlas) -> usize {
        self.atlases.add(atlas)
    }

    /// Loads an atlas from the PNG asset `asset` and adds it to the scene. When `name` is `None`
    /// or empty, the asset's file name is used.
    pub fn create_atlas_from_png(
        &mut self,
        assets: &dyn AssetsProvider,
        asset: &str,
        name: Option<String>,
    ) -> Result<usize> {
        let atlas = Atlas::new_from_png(assets, asset, name)?;
        Ok(self.add_atlas(atlas))
    }

    pub fn get_atlas(&self, handle: usize) -> Option<&Atlas> {
        self.atlases.get(handle)
    }

    pub fn get_texture(&self, handle: usize) -> Option<&Texture> {
        self.textures.get(handle)
    }

    /// Adds a texture and registers it with its atlas. Fails if the atlas does not exist.
    pub fn add_texture(&mut self, texture: Texture) -> Result<usize> {
        let atlas_handle = texture.get_atlas();
        if self.atlases.get(atlas_handle).is_none() {
            anyhow::bail!("Atlas {atlas_handle} not found");
        }
        let texture_handle = self.textures.add(texture);
        self.atlases
            .get_mut(atlas_handle)
            .unwrap()
            .add_texture(texture_handle);
        Ok(texture_handle)
    }

    /// Creates a texture covering the whole atlas and makes it the atlas's full texture.
    pub fn create_full_atlas_texture(
        &mut self,
        name: Option<String>,
        atlas_handle: usize,
    ) -> Result<usize> {
        let texture_handle = self.add_texture(Texture::full_atlas(name, atlas_handle))?;
        self.atlases
            .get_mut(atlas_handle)
            .unwrap()
            .set_full_texture(texture_handle);
        Ok(texture_handle)
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
    use crate::feather::uniform::UniformValue;
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
        assert!(node.get_children().is_empty());
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
        assert!(root_node.get_children().contains(&child));
        assert_eq!(root_node.get_children().len(), 1);
    }

    #[test]
    fn create_multiple_children() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let a = scene.create_node(Some("a".to_string()), root).unwrap();
        let b = scene.create_node(Some("b".to_string()), root).unwrap();
        let c = scene.create_node(None, root).unwrap();

        let children = scene.get_node(root).unwrap().get_children();
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
            .get_children()
            .contains(&grandchild));
        assert!(!scene
            .get_node(root)
            .unwrap()
            .get_children()
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

        let children = scene.get_node(root).unwrap().get_children();
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
        assert_eq!(scene.get_node(root).unwrap().get_children().len(), 2);
        // The first node keeps its own child.
        assert!(scene
            .get_node(first)
            .unwrap()
            .get_children()
            .contains(&grandchild));
        assert!(scene.get_node(second).unwrap().get_children().is_empty());
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
            .get_children()
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

    // --- shaders and materials ---

    use crate::feather::assetsprovider::tests::test_resources;
    use crate::feather::shader::tests::{FRAG, VERT};

    #[test]
    fn create_shader_adds_it() {
        let mut scene = Scene::new();
        let shader = scene
            .create_shader(
                Some("basic".to_string()),
                VERT,
                FRAG,
                Some("model".to_string()),
                Some("view".to_string()),
                Some("proj".to_string()),
            )
            .unwrap();

        let shader_obj = scene.get_shader(shader).unwrap();
        assert_eq!(shader_obj.get_handle(), shader);
        assert_eq!(shader_obj.get_vertex(), VERT);
        assert_eq!(shader_obj.get_uniform_model_name(), Some("model"));
        assert_eq!(shader_obj.get_uniform_view_name(), Some("view"));
        assert_eq!(shader_obj.get_uniform_proj_name(), Some("proj"));
        assert_eq!(
            scene.shaders.get_by_name("basic").unwrap().get_handle(),
            shader
        );
    }

    #[test]
    fn create_shader_with_invalid_code_fails() {
        let mut scene = Scene::new();
        assert!(scene
            .create_shader(None, &[1, 2, 3], FRAG, None, None, None)
            .is_err());
        assert_eq!(scene.shaders.iter().count(), 0);
    }

    #[test]
    fn create_material_uses_shader() {
        let mut scene = Scene::new();
        let shader = scene
            .create_shader(None, VERT, FRAG, None, None, None)
            .unwrap();
        let material = scene
            .create_material(Some("wood".to_string()), shader)
            .unwrap();

        let material_obj = scene.get_material(material).unwrap();
        assert_eq!(material_obj.get_handle(), material);
        assert_eq!(material_obj.get_shader(), shader);
        assert_eq!(material_obj.get_name(), Some("wood".to_string()));
    }

    #[test]
    fn create_material_with_invalid_shader_fails() {
        let mut scene = Scene::new();
        assert!(scene.create_material(None, 42).is_err());
        assert_eq!(scene.materials.iter().count(), 0);
    }

    #[test]
    fn material_uniforms_can_hold_texture() {
        let mut scene = Scene::new();
        let atlas = scene.add_atlas(test_atlas(None));
        let texture = scene.create_full_atlas_texture(None, atlas).unwrap();
        let shader = scene
            .create_shader(None, VERT, FRAG, None, None, None)
            .unwrap();
        let material = scene.create_material(None, shader).unwrap();

        scene
            .get_material_mut(material)
            .unwrap()
            .uniforms_mut()
            .set("texSampler", UniformValue::Texture(texture));

        assert_eq!(
            scene
                .get_material(material)
                .unwrap()
                .uniforms()
                .get("texSampler"),
            Some(&UniformValue::Texture(texture))
        );
    }

    #[test]
    fn material_set_shader() {
        let mut scene = Scene::new();
        let s1 = scene
            .create_shader(None, VERT, FRAG, None, None, None)
            .unwrap();
        let s2 = scene
            .create_shader(None, VERT, FRAG, None, None, None)
            .unwrap();
        let material = scene.create_material(None, s1).unwrap();

        scene.material_set_shader(material, s2).unwrap();
        assert_eq!(scene.get_material(material).unwrap().get_shader(), s2);

        assert!(scene.material_set_shader(material, 42).is_err());
        assert!(scene.material_set_shader(42, s1).is_err());
        assert_eq!(scene.get_material(material).unwrap().get_shader(), s2);
    }

    #[test]
    fn node_set_material() {
        let mut scene = Scene::new();
        let node = scene.create_root_node(None);
        let shader = scene
            .create_shader(None, VERT, FRAG, None, None, None)
            .unwrap();
        let material = scene.create_material(None, shader).unwrap();

        scene.node_set_material(node, material).unwrap();
        assert_eq!(scene.get_node(node).unwrap().get_material(), Some(material));
    }

    #[test]
    fn node_set_material_with_invalid_handles_fails() {
        let mut scene = Scene::new();
        let node = scene.create_root_node(None);
        let shader = scene
            .create_shader(None, VERT, FRAG, None, None, None)
            .unwrap();
        let material = scene.create_material(None, shader).unwrap();

        assert!(scene.node_set_material(node, 42).is_err());
        assert!(scene.node_set_material(42, material).is_err());
        assert_eq!(scene.get_node(node).unwrap().get_material(), None);
    }

    #[test]
    fn materials_are_shared_between_nodes() {
        let mut scene = Scene::new();
        let a = scene.create_root_node(None);
        let b = scene.create_root_node(None);
        let shader = scene
            .create_shader(None, VERT, FRAG, None, None, None)
            .unwrap();
        let material = scene.create_material(None, shader).unwrap();

        scene.node_set_material(a, material).unwrap();
        scene.node_set_material(b, material).unwrap();

        assert_eq!(scene.get_node(a).unwrap().get_material(), Some(material));
        assert_eq!(scene.get_node(b).unwrap().get_material(), Some(material));
        assert_eq!(scene.materials.iter().count(), 1);
    }

    // --- dynamic uniforms ---

    #[test]
    fn dynamic_uniforms_are_inherited_through_hierarchy() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let child = scene.create_node(None, root).unwrap();
        let grandchild = scene.create_node(None, child).unwrap();
        scene
            .get_node_mut(root)
            .unwrap()
            .uniforms_mut()
            .set("a", 1u32);
        scene
            .get_node_mut(child)
            .unwrap()
            .uniforms_mut()
            .set("b", 2u32);

        scene.update_dynamic_uniforms();

        let dynamic = scene.get_node(grandchild).unwrap().dynamic_uniforms();
        assert_eq!(dynamic.get("a"), Some(&UniformValue::UInt(1)));
        assert_eq!(dynamic.get("b"), Some(&UniformValue::UInt(2)));
        assert_eq!(dynamic.len(), 2);

        // A parent does not see its children's uniforms.
        assert_eq!(
            scene.get_node(root).unwrap().dynamic_uniforms().get("b"),
            None
        );
    }

    #[test]
    fn own_uniforms_override_inherited_ones() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let child = scene.create_node(None, root).unwrap();
        let grandchild = scene.create_node(None, child).unwrap();
        scene
            .get_node_mut(root)
            .unwrap()
            .uniforms_mut()
            .set("a", 1u32);
        scene
            .get_node_mut(child)
            .unwrap()
            .uniforms_mut()
            .set("a", 10u32);

        scene.update_dynamic_uniforms();

        assert_eq!(
            scene.get_node(root).unwrap().dynamic_uniforms().get("a"),
            Some(&UniformValue::UInt(1))
        );
        assert_eq!(
            scene.get_node(child).unwrap().dynamic_uniforms().get("a"),
            Some(&UniformValue::UInt(10))
        );
        // The override is passed further down.
        assert_eq!(
            scene
                .get_node(grandchild)
                .unwrap()
                .dynamic_uniforms()
                .get("a"),
            Some(&UniformValue::UInt(10))
        );
    }

    #[test]
    fn siblings_do_not_affect_each_other() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let a = scene.create_node(None, root).unwrap();
        let b = scene.create_node(None, root).unwrap();
        scene
            .get_node_mut(root)
            .unwrap()
            .uniforms_mut()
            .set("x", 1u32);
        scene.get_node_mut(a).unwrap().uniforms_mut().set("x", 2u32);

        scene.update_dynamic_uniforms();

        assert_eq!(
            scene.get_node(a).unwrap().dynamic_uniforms().get("x"),
            Some(&UniformValue::UInt(2))
        );
        assert_eq!(
            scene.get_node(b).unwrap().dynamic_uniforms().get("x"),
            Some(&UniformValue::UInt(1))
        );
    }

    #[test]
    fn separate_roots_do_not_share_uniforms() {
        let mut scene = Scene::new();
        let r1 = scene.create_root_node(None);
        let r2 = scene.create_root_node(None);
        scene
            .get_node_mut(r1)
            .unwrap()
            .uniforms_mut()
            .set("x", 1u32);

        scene.update_dynamic_uniforms();

        assert!(scene.get_node(r2).unwrap().dynamic_uniforms().is_empty());
    }

    #[test]
    fn update_node_dynamic_uniforms_updates_subtree_only() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let a = scene.create_node(None, root).unwrap();
        let a_child = scene.create_node(None, a).unwrap();
        let b = scene.create_node(None, root).unwrap();
        scene
            .get_node_mut(root)
            .unwrap()
            .uniforms_mut()
            .set("x", 1u32);
        scene.update_dynamic_uniforms();

        scene.get_node_mut(a).unwrap().uniforms_mut().set("y", 2u32);
        scene.get_node_mut(b).unwrap().uniforms_mut().set("y", 3u32);
        scene.update_node_dynamic_uniforms(a).unwrap();

        // `a` and its child are updated and still inherit from the root.
        let a_child_dynamic = scene.get_node(a_child).unwrap().dynamic_uniforms();
        assert_eq!(a_child_dynamic.get("x"), Some(&UniformValue::UInt(1)));
        assert_eq!(a_child_dynamic.get("y"), Some(&UniformValue::UInt(2)));
        // `b` is outside the subtree, so it is not updated yet.
        assert_eq!(scene.get_node(b).unwrap().dynamic_uniforms().get("y"), None);
    }

    #[test]
    fn update_node_dynamic_uniforms_with_invalid_handle_fails() {
        let mut scene = Scene::new();
        assert!(scene.update_node_dynamic_uniforms(42).is_err());
    }

    #[test]
    fn disconnected_node_stops_inheriting() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let child = scene.create_node(None, root).unwrap();
        scene
            .get_node_mut(root)
            .unwrap()
            .uniforms_mut()
            .set("x", 1u32);
        scene
            .get_node_mut(child)
            .unwrap()
            .uniforms_mut()
            .set("y", 2u32);
        scene.update_dynamic_uniforms();

        scene.disconnect_node(child).unwrap();
        scene.update_dynamic_uniforms();

        let dynamic = scene.get_node(child).unwrap().dynamic_uniforms();
        assert_eq!(dynamic.get("x"), None);
        assert_eq!(dynamic.get("y"), Some(&UniformValue::UInt(2)));
    }

    /// Creates a material with one uniform and returns its handle.
    fn material_with(scene: &mut Scene, name: &str, value: u32) -> usize {
        let shader = scene
            .create_shader(None, VERT, FRAG, None, None, None)
            .unwrap();
        let material = scene.create_material(None, shader).unwrap();
        scene
            .get_material_mut(material)
            .unwrap()
            .uniforms_mut()
            .set(name, value);
        material
    }

    #[test]
    fn material_uniforms_are_part_of_dynamic_uniforms() {
        let mut scene = Scene::new();
        let node = scene.create_root_node(None);
        let material = material_with(&mut scene, "m", 1);
        scene.node_set_material(node, material).unwrap();

        scene.update_dynamic_uniforms();

        assert_eq!(
            scene.get_node(node).unwrap().dynamic_uniforms().get("m"),
            Some(&UniformValue::UInt(1))
        );
    }

    #[test]
    fn node_uniforms_override_material_uniforms() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let child = scene.create_node(None, root).unwrap();
        let material = material_with(&mut scene, "x", 1);
        scene.node_set_material(child, material).unwrap();
        scene.node_set_material(root, material).unwrap();
        // Own uniform beats the root's material; inherited beats the child's material.
        scene
            .get_node_mut(root)
            .unwrap()
            .uniforms_mut()
            .set("x", 2u32);

        scene.update_dynamic_uniforms();

        assert_eq!(
            scene.get_node(root).unwrap().dynamic_uniforms().get("x"),
            Some(&UniformValue::UInt(2))
        );
        assert_eq!(
            scene.get_node(child).unwrap().dynamic_uniforms().get("x"),
            Some(&UniformValue::UInt(2))
        );
    }

    #[test]
    fn material_uniforms_are_not_inherited() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let child = scene.create_node(None, root).unwrap();
        let root_material = material_with(&mut scene, "texSampler", 1);
        let child_material = material_with(&mut scene, "texSampler", 2);
        scene.node_set_material(root, root_material).unwrap();
        scene.node_set_material(child, child_material).unwrap();
        let grandchild = scene.create_node(None, child).unwrap();

        scene.update_dynamic_uniforms();

        // The child uses its own material, not the root's.
        assert_eq!(
            scene
                .get_node(child)
                .unwrap()
                .dynamic_uniforms()
                .get("texSampler"),
            Some(&UniformValue::UInt(2))
        );
        // A node without a material gets nothing from its ancestors' materials.
        assert_eq!(
            scene
                .get_node(grandchild)
                .unwrap()
                .dynamic_uniforms()
                .get("texSampler"),
            None
        );
    }

    #[test]
    fn update_node_dynamic_uniforms_picks_up_material_change() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let child = scene.create_node(None, root).unwrap();
        scene
            .get_node_mut(root)
            .unwrap()
            .uniforms_mut()
            .set("a", 1u32);
        let material = material_with(&mut scene, "m", 1);
        scene.node_set_material(child, material).unwrap();
        scene.update_dynamic_uniforms();

        scene
            .get_material_mut(material)
            .unwrap()
            .uniforms_mut()
            .set("m", 5u32);
        scene.update_node_dynamic_uniforms(child).unwrap();

        let dynamic = scene.get_node(child).unwrap().dynamic_uniforms();
        assert_eq!(dynamic.get("m"), Some(&UniformValue::UInt(5)));
        // Still inherits from the parent.
        assert_eq!(dynamic.get("a"), Some(&UniformValue::UInt(1)));
    }

    #[test]
    fn update_node_dynamic_uniforms_does_not_inherit_parent_material() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let child = scene.create_node(None, root).unwrap();
        let material = material_with(&mut scene, "m", 1);
        scene.node_set_material(root, material).unwrap();
        scene.update_dynamic_uniforms();

        scene.update_node_dynamic_uniforms(child).unwrap();

        assert_eq!(
            scene.get_node(child).unwrap().dynamic_uniforms().get("m"),
            None
        );
    }

    // --- atlases and textures ---

    /// 1x1 in-memory atlas.
    fn test_atlas(name: Option<&str>) -> Atlas {
        Atlas::from_rgba(name.map(str::to_string), 1, 1, vec![0, 0, 0, 255])
    }

    #[test]
    fn create_atlas_from_png_adds_atlas() {
        let mut scene = Scene::new();
        let atlas = scene
            .create_atlas_from_png(&*test_resources(), "texture.png", None)
            .unwrap();

        let atlas_obj = scene.get_atlas(atlas).unwrap();
        assert_eq!(atlas_obj.get_handle(), atlas);
        assert_eq!(atlas_obj.get_name(), Some("texture.png".to_string()));
        assert_eq!((atlas_obj.get_width(), atlas_obj.get_height()), (512, 512));
        assert_eq!(
            scene
                .atlases
                .get_by_name("texture.png")
                .unwrap()
                .get_handle(),
            atlas
        );
    }

    #[test]
    fn create_atlas_from_png_with_name() {
        let mut scene = Scene::new();
        let atlas = scene
            .create_atlas_from_png(&*test_resources(), "texture.png", Some("bricks".to_string()))
            .unwrap();
        assert_eq!(
            scene.get_atlas(atlas).unwrap().get_name(),
            Some("bricks".to_string())
        );
    }

    #[test]
    fn create_atlas_from_png_with_missing_file_fails() {
        let mut scene = Scene::new();
        assert!(scene
            .create_atlas_from_png(&*test_resources(), "does_not_exist.png", None)
            .is_err());
        assert_eq!(scene.atlases.iter().count(), 0);
    }

    #[test]
    fn full_texture_of_loaded_atlas() {
        let mut scene = Scene::new();
        let atlas = scene
            .create_atlas_from_png(&*test_resources(), "texture.png", None)
            .unwrap();
        let full = scene.create_full_atlas_texture(None, atlas).unwrap();

        assert_eq!(
            scene.get_atlas(atlas).unwrap().get_full_texture(),
            Some(full)
        );
        assert_eq!(scene.get_texture(full).unwrap().get_atlas(), atlas);
    }

    #[test]
    fn add_texture_registers_it_with_atlas() {
        let mut scene = Scene::new();
        let atlas = scene.add_atlas(test_atlas(None));
        let t1 = scene
            .add_texture(Texture::full_atlas(Some("t1".to_string()), atlas))
            .unwrap();
        let t2 = scene.add_texture(Texture::full_atlas(None, atlas)).unwrap();

        assert_ne!(t1, t2);
        assert_eq!(scene.get_texture(t1).unwrap().get_handle(), t1);
        assert_eq!(scene.get_texture(t1).unwrap().get_atlas(), atlas);
        assert_eq!(scene.get_atlas(atlas).unwrap().get_textures(), &[t1, t2]);
        // Plain textures do not become the full texture.
        assert_eq!(scene.get_atlas(atlas).unwrap().get_full_texture(), None);
    }

    #[test]
    fn add_texture_with_invalid_atlas_fails() {
        let mut scene = Scene::new();
        assert!(scene.add_texture(Texture::full_atlas(None, 42)).is_err());
        assert_eq!(scene.textures.iter().count(), 0);
    }

    #[test]
    fn create_full_atlas_texture() {
        let mut scene = Scene::new();
        let atlas = scene.add_atlas(test_atlas(None));
        let other = scene.add_texture(Texture::full_atlas(None, atlas)).unwrap();
        let full = scene.create_full_atlas_texture(None, atlas).unwrap();

        let atlas_obj = scene.get_atlas(atlas).unwrap();
        assert_eq!(atlas_obj.get_full_texture(), Some(full));
        assert_eq!(atlas_obj.get_textures(), &[other, full]);
        assert_eq!(scene.get_texture(full).unwrap().get_atlas(), atlas);
    }

    #[test]
    fn create_full_atlas_texture_with_invalid_atlas_fails() {
        let mut scene = Scene::new();
        assert!(scene.create_full_atlas_texture(None, 42).is_err());
        assert_eq!(scene.textures.iter().count(), 0);
    }

    #[test]
    fn textures_stay_with_their_own_atlas() {
        let mut scene = Scene::new();
        let a = scene.add_atlas(test_atlas(Some("a")));
        let b = scene.add_atlas(test_atlas(Some("b")));
        let ta = scene.create_full_atlas_texture(None, a).unwrap();
        let tb = scene.create_full_atlas_texture(None, b).unwrap();

        assert_eq!(scene.get_atlas(a).unwrap().get_textures(), &[ta]);
        assert_eq!(scene.get_atlas(b).unwrap().get_textures(), &[tb]);
        assert_eq!(scene.get_atlas(a).unwrap().get_full_texture(), Some(ta));
        assert_eq!(scene.get_atlas(b).unwrap().get_full_texture(), Some(tb));
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
