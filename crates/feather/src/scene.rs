use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;

use crate::{
    assetsprovider::AssetsProvider,
    atlas::Atlas,
    material::Material,
    object::Object,
    shader::{Shader, ShaderUniformNames},
    texture::Texture,
};

use super::{
    math::Mat4, mesh::Mesh, meshbuffer::MeshBuffer, node::Node, objdb::ObjDB, uniforms::Uniforms,
};

/// One draw call: a node's mesh, located in a mesh buffer, drawn with the node's material (its
/// shader and texture) and the node's model matrix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawItem {
    pub node: usize,
    /// Whether the node is transparent (`Node::is_transparent`): blended, and drawn after the
    /// opaque nodes, back to front.
    pub transparent: bool,
    /// Handle of the node's material (in `Scene::materials`).
    pub material: usize,
    /// Handle of the shader (in `Scene::shaders`) of the node's material.
    pub shader: usize,
    /// Handle of the mesh buffer (in `Scene::buffers`) holding the mesh.
    pub mesh_buffer: usize,
    pub model: Mat4,
    /// First index of the mesh in the buffer's index buffer.
    pub first_index: u32,
    pub index_count: u32,
    /// Added to every index: where the mesh's vertices start in the buffer's vertex buffer.
    pub vertex_offset: i32,
}

/// Puts draw items in the order to draw them:
/// 1. opaque items, grouped by shader so the renderer switches pipelines as little as possible;
/// 2. transparent items, from the farthest to the nearest, so each is blended over everything
///    behind it.
///
/// Distances are measured along the view direction of `view` (the camera's view matrix) to each
/// node's origin. Items at the same place keep their order.
pub fn sort_for_rendering(items: &mut [DrawItem], view: &Mat4) {
    // Distance in front of the camera; the camera looks down -Z in view space.
    let depth = |item: &DrawItem| -(view * item.model.w).z;
    items.sort_by(|a, b| match (a.transparent, b.transparent) {
        (false, false) => a.shader.cmp(&b.shader),
        (false, true) => std::cmp::Ordering::Less,
        (true, false) => std::cmp::Ordering::Greater,
        (true, true) => depth(b).total_cmp(&depth(a)),
    });
}

/// Source of `Scene::get_id` values.
static NEXT_SCENE_ID: AtomicU64 = AtomicU64::new(0);

pub struct Scene {
    /// Unique among all scenes created by the process.
    id: u64,
    pub meshes: ObjDB<Mesh>,
    pub buffers: ObjDB<MeshBuffer>,
    pub nodes: ObjDB<Node>,
    pub atlases: ObjDB<Atlas>,
    pub shaders: ObjDB<Shader>,
    pub materials: ObjDB<Material>,
    needs_create_mesh_buffer: bool,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            id: NEXT_SCENE_ID.fetch_add(1, Ordering::Relaxed),
            meshes: ObjDB::new(),
            buffers: ObjDB::new(),
            nodes: ObjDB::new(),
            atlases: ObjDB::new(),
            shaders: ObjDB::new(),
            materials: ObjDB::new(),
            needs_create_mesh_buffer: false,
        }
    }

    /// Identifies the scene: no two scenes created by the process have the same id. The
    /// renderer uses it to notice when it is given a different scene, whose handles mean
    /// different objects.
    pub fn get_id(&self) -> u64 {
        self.id
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
    /// See `Shader::new` and `ShaderUniformNames` for `uniform_names`.
    pub fn create_shader(
        &mut self,
        name: Option<String>,
        vertex: &[u8],
        fragment: &[u8],
        uniform_names: ShaderUniformNames,
    ) -> Result<usize> {
        let shader = Shader::new(name, vertex, fragment, uniform_names)?;
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

    /// Sets the texture of a material (`None` for no texture). Fails if the material or the
    /// texture's atlas does not exist.
    pub fn material_set_texture(
        &mut self,
        material_handle: usize,
        texture: Option<Texture>,
    ) -> Result<()> {
        if let Some(texture) = &texture {
            let atlas = texture.get_atlas();
            if self.atlases.get(atlas).is_none() {
                anyhow::bail!("Atlas {atlas} not found");
            }
        }
        self.materials
            .get_mut(material_handle)
            .ok_or_else(|| anyhow::anyhow!("Material {material_handle} not found"))?
            .set_texture(texture);
        Ok(())
    }

    fn root_nodes(&self) -> Vec<usize> {
        self.nodes
            .iter()
            .filter(|n| n.is_root())
            .map(|n| n.get_handle())
            .collect()
    }

    /// Recomputes the dynamic uniforms and global transforms of every node, starting from the
    /// root nodes.
    pub fn update_nodes(&mut self) {
        for root in self.root_nodes() {
            self.update_subtree(root, &Uniforms::new(), None);
        }
    }

    /// Recomputes the dynamic uniforms and global transforms of a node and all its
    /// descendants. Use it after changing one node (its transform, uniforms or material);
    /// the node's parent must already be up to date.
    pub fn update_node(&mut self, node_handle: usize) -> Result<()> {
        let node = self
            .nodes
            .get(node_handle)
            .ok_or_else(|| anyhow::anyhow!("Node {node_handle} not found"))?;
        let parent = node.get_parent().and_then(|p| self.nodes.get(p));
        let inherited = parent.map(|p| p.uniforms_for_children()).unwrap_or_default();
        let parent_transform = parent.map(|p| p.get_global_transform().unwrap_or(p.get_transform()));
        self.update_subtree(node_handle, &inherited, parent_transform);
        Ok(())
    }

    fn update_subtree(
        &mut self,
        node_handle: usize,
        inherited: &Uniforms,
        parent_transform: Option<Mat4>,
    ) {
        // Depth-first, without recursion:
        // (node, uniforms inherited from its parent, parent's global transform).
        let mut stack = vec![(node_handle, inherited.clone(), parent_transform)];
        while let Some((handle, inherited, parent_transform)) = stack.pop() {
            let Some(node) = self.nodes.get_mut(handle) else {
                continue;
            };
            let material = node
                .get_material()
                .and_then(|m| self.materials.get(m))
                .map(|m| m.uniforms());
            node.update_dynamic_uniforms(&inherited, material);
            node.update_global_transform(parent_transform.as_ref());

            let for_children = node.uniforms_for_children();
            let global = node.get_global_transform();
            for &child in node.get_children() {
                stack.push((child, for_children.clone(), global));
            }
        }
    }

    /// Lists what to draw: every visible node that has a material with an existing shader and
    /// a mesh already placed in a mesh buffer. A hidden node hides its descendants too. Uses
    /// the global transforms computed by `update_nodes`.
    pub fn draw_items(&self) -> Vec<DrawItem> {
        let mut items = Vec::new();
        let mut stack = self.root_nodes();
        while let Some(handle) = stack.pop() {
            let Some(node) = self.nodes.get(handle) else {
                continue;
            };
            if !node.is_visible() {
                continue;
            }
            stack.extend(node.get_children());

            let Some(data) = node
                .get_mesh()
                .and_then(|m| self.meshes.get(m))
                .and_then(|m| m.mesh_buffer_data.as_ref())
            else {
                continue;
            };
            let Some(material) = node.get_material().and_then(|m| self.materials.get(m)) else {
                continue;
            };
            let shader = material.get_shader();
            if self.shaders.get(shader).is_none() {
                continue;
            }
            items.push(DrawItem {
                node: handle,
                transparent: node.is_transparent(),
                material: material.get_handle(),
                shader,
                mesh_buffer: data.buffer_handle,
                model: node.get_global_transform().unwrap_or(node.get_transform()),
                first_index: data.index_begin_index as u32,
                index_count: data.index_size as u32,
                vertex_offset: data.vertex_begin_index as i32,
            });
        }
        items
    }

    pub fn add_atlas(&mut self, atlas: Atlas) -> usize {
        self.atlases.add(atlas)
    }

    /// Loads an atlas from the PNG asset `bitmap` and, when given, its metadata asset
    /// `metadata` (a `.atlas` file), and adds it to the scene. When `name` is `None` or empty,
    /// the bitmap's file name is used. See `Atlas::new_from_png`.
    pub fn create_atlas_from_png(
        &mut self,
        assets: &dyn AssetsProvider,
        bitmap: &str,
        metadata: Option<&str>,
        name: Option<String>,
    ) -> Result<usize> {
        let atlas = Atlas::new_from_png(assets, bitmap, metadata, name)?;
        Ok(self.add_atlas(atlas))
    }

    pub fn get_atlas(&self, handle: usize) -> Option<&Atlas> {
        self.atlases.get(handle)
    }

    /// Handle of the first atlas named `name`.
    pub fn find_atlas(&self, name: &str) -> Option<usize> {
        self.atlases.get_by_name(name).map(|a| a.get_handle())
    }

    /// Handle of the first shader named `name`.
    pub fn find_shader(&self, name: &str) -> Option<usize> {
        self.shaders.get_by_name(name).map(|s| s.get_handle())
    }

    /// Handle of the first mesh named `name`.
    pub fn find_mesh(&self, name: &str) -> Option<usize> {
        self.meshes.get_by_name(name).map(|m| m.get_handle())
    }

    /// Handle of the first material named `name`.
    pub fn find_material(&self, name: &str) -> Option<usize> {
        self.materials.get_by_name(name).map(|m| m.get_handle())
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
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 512x512 PNG in testdata/.
    const TEST_PNG: &str = "atlas_512.png";
    use crate::math::{Vec2, Vec3};
    use crate::uniform::UniformValue;
    use crate::vertex::Vertex;

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
    fn scenes_have_unique_ids() {
        let a = Scene::new();
        let b = Scene::new();
        let c = Scene::default();
        assert_ne!(a.get_id(), b.get_id());
        assert_ne!(b.get_id(), c.get_id());
        assert_ne!(a.get_id(), c.get_id());
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

    use crate::assetsprovider::tests::test_resources;
    use crate::shader::tests::{FRAG, VERT};

    #[test]
    fn create_shader_adds_it() {
        let mut scene = Scene::new();
        let shader = scene
            .create_shader(
                Some("basic".to_string()),
                VERT,
                FRAG,
                ShaderUniformNames::matrices("model", "view", "proj"),
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
            .create_shader(None, &[1, 2, 3], FRAG, ShaderUniformNames::default())
            .is_err());
        assert_eq!(scene.shaders.iter().count(), 0);
    }

    #[test]
    fn create_material_uses_shader() {
        let mut scene = Scene::new();
        let shader = scene
            .create_shader(None, VERT, FRAG, ShaderUniformNames::default())
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
    fn material_set_texture() {
        let mut scene = Scene::new();
        let atlas = scene.add_atlas(test_atlas(None));
        let shader = scene
            .create_shader(None, VERT, FRAG, ShaderUniformNames::default())
            .unwrap();
        let material = scene.create_material(None, shader).unwrap();

        let texture = scene.get_atlas(atlas).unwrap().get_full_texture();
        assert_eq!(texture.get_atlas(), atlas);
        scene.material_set_texture(material, Some(texture)).unwrap();
        assert_eq!(
            scene.get_material(material).unwrap().get_texture(),
            Some(&texture)
        );

        scene.material_set_texture(material, None).unwrap();
        assert_eq!(scene.get_material(material).unwrap().get_texture(), None);
    }

    #[test]
    fn material_set_texture_with_invalid_handles_fails() {
        let mut scene = Scene::new();
        let atlas = scene.add_atlas(test_atlas(None));
        let shader = scene
            .create_shader(None, VERT, FRAG, ShaderUniformNames::default())
            .unwrap();
        let material = scene.create_material(None, shader).unwrap();

        assert!(scene.material_set_texture(material, Some(Texture::full(42))).is_err());
        assert!(scene.material_set_texture(42, Some(Texture::full(atlas))).is_err());
        assert_eq!(scene.get_material(material).unwrap().get_texture(), None);
    }

    #[test]
    fn material_set_shader() {
        let mut scene = Scene::new();
        let s1 = scene
            .create_shader(None, VERT, FRAG, ShaderUniformNames::default())
            .unwrap();
        let s2 = scene
            .create_shader(None, VERT, FRAG, ShaderUniformNames::default())
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
            .create_shader(None, VERT, FRAG, ShaderUniformNames::default())
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
            .create_shader(None, VERT, FRAG, ShaderUniformNames::default())
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
            .create_shader(None, VERT, FRAG, ShaderUniformNames::default())
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

        scene.update_nodes();

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

        scene.update_nodes();

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

        scene.update_nodes();

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

        scene.update_nodes();

        assert!(scene.get_node(r2).unwrap().dynamic_uniforms().is_empty());
    }

    #[test]
    fn update_node_updates_subtree_only() {
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
        scene.update_nodes();

        scene.get_node_mut(a).unwrap().uniforms_mut().set("y", 2u32);
        scene.get_node_mut(b).unwrap().uniforms_mut().set("y", 3u32);
        scene.update_node(a).unwrap();

        // `a` and its child are updated and still inherit from the root.
        let a_child_dynamic = scene.get_node(a_child).unwrap().dynamic_uniforms();
        assert_eq!(a_child_dynamic.get("x"), Some(&UniformValue::UInt(1)));
        assert_eq!(a_child_dynamic.get("y"), Some(&UniformValue::UInt(2)));
        // `b` is outside the subtree, so it is not updated yet.
        assert_eq!(scene.get_node(b).unwrap().dynamic_uniforms().get("y"), None);
    }

    #[test]
    fn update_node_with_invalid_handle_fails() {
        let mut scene = Scene::new();
        assert!(scene.update_node(42).is_err());
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
        scene.update_nodes();

        scene.disconnect_node(child).unwrap();
        scene.update_nodes();

        let dynamic = scene.get_node(child).unwrap().dynamic_uniforms();
        assert_eq!(dynamic.get("x"), None);
        assert_eq!(dynamic.get("y"), Some(&UniformValue::UInt(2)));
    }

    /// Creates a material with one uniform and returns its handle.
    fn material_with(scene: &mut Scene, name: &str, value: u32) -> usize {
        let shader = scene
            .create_shader(None, VERT, FRAG, ShaderUniformNames::default())
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

        scene.update_nodes();

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

        scene.update_nodes();

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

        scene.update_nodes();

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
    fn update_node_picks_up_material_change() {
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
        scene.update_nodes();

        scene
            .get_material_mut(material)
            .unwrap()
            .uniforms_mut()
            .set("m", 5u32);
        scene.update_node(child).unwrap();

        let dynamic = scene.get_node(child).unwrap().dynamic_uniforms();
        assert_eq!(dynamic.get("m"), Some(&UniformValue::UInt(5)));
        // Still inherits from the parent.
        assert_eq!(dynamic.get("a"), Some(&UniformValue::UInt(1)));
    }

    #[test]
    fn update_node_does_not_inherit_parent_material() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let child = scene.create_node(None, root).unwrap();
        let material = material_with(&mut scene, "m", 1);
        scene.node_set_material(root, material).unwrap();
        scene.update_nodes();

        scene.update_node(child).unwrap();

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
            .create_atlas_from_png(&*test_resources(), TEST_PNG, None, None)
            .unwrap();

        let atlas_obj = scene.get_atlas(atlas).unwrap();
        assert_eq!(atlas_obj.get_handle(), atlas);
        assert_eq!(atlas_obj.get_name(), Some(TEST_PNG.to_string()));
        assert_eq!((atlas_obj.get_width(), atlas_obj.get_height()), (512, 512));
        assert_eq!(
            scene
                .atlases
                .get_by_name(TEST_PNG)
                .unwrap()
                .get_handle(),
            atlas
        );
    }

    #[test]
    fn create_atlas_from_png_with_name() {
        let mut scene = Scene::new();
        let atlas = scene
            .create_atlas_from_png(&*test_resources(), TEST_PNG, None, Some("bricks".to_string()))
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
            .create_atlas_from_png(&*test_resources(), "does_not_exist.png", None, None)
            .is_err());
        assert_eq!(scene.atlases.iter().count(), 0);
    }

    #[test]
    fn textures_of_an_atlas_in_the_scene_refer_to_it() {
        let mut scene = Scene::new();
        scene.add_atlas(test_atlas(Some("first")));
        let mut atlas = Atlas::from_rgba(Some("b".to_string()), 4, 4, vec![0; 64]);
        atlas.set_metadata("quarter 0 0 2 2").unwrap();
        let b = scene.add_atlas(atlas);

        let atlas = scene.get_atlas(b).unwrap();
        assert_eq!(atlas.get_full_texture(), Texture::full(b));
        assert_eq!(
            atlas.get_texture("quarter"),
            Some(Texture::new(b, 0.0, 0.5, 0.0, 0.5))
        );
    }

    #[test]
    fn find_objects_by_name() {
        let mut scene = Scene::new();
        let atlas = scene.add_atlas(test_atlas(Some("atlas")));
        let shader = scene
            .create_shader(Some("shader".to_string()), VERT, FRAG, ShaderUniformNames::default())
            .unwrap();
        let material = scene
            .create_material(Some("material".to_string()), shader)
            .unwrap();
        let mesh = scene.add_mesh(Mesh::new(Some("mesh".to_string()), Vec::new(), Vec::new()));

        assert_eq!(scene.find_atlas("atlas"), Some(atlas));
        assert_eq!(scene.find_shader("shader"), Some(shader));
        assert_eq!(scene.find_material("material"), Some(material));
        assert_eq!(scene.find_mesh("mesh"), Some(mesh));
        assert_eq!(scene.find_atlas("shader"), None);
        assert_eq!(scene.find_mesh("missing"), None);
    }

    // --- mesh buffers ---

    #[test]
    fn create_mesh_buffer_returns_distinct_handles() {
        let mut scene = Scene::new();
        let b0 = scene.create_mesh_buffer();
        let b1 = scene.create_mesh_buffer();

        assert_ne!(b0, b1);
        assert_eq!(scene.buffers.iter().count(), 2);
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

    // --- transforms and drawing ---

    fn translation(x: f32, y: f32, z: f32) -> Mat4 {
        Mat4::from_translation(Vec3::new(x, y, z))
    }

    /// Creates a shader and a material using it; returns (shader, material).
    fn test_material(scene: &mut Scene) -> (usize, usize) {
        let shader = scene.create_shader(None, VERT, FRAG, ShaderUniformNames::default()).unwrap();
        let material = scene.create_material(None, shader).unwrap();
        (shader, material)
    }

    #[test]
    fn update_nodes_computes_global_transforms() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let child = scene.create_node(None, root).unwrap();
        let grandchild = scene.create_node(None, child).unwrap();
        scene.get_node_mut(root).unwrap().set_transform(translation(1.0, 0.0, 0.0));
        scene.get_node_mut(child).unwrap().set_transform(translation(0.0, 2.0, 0.0));
        scene.get_node_mut(grandchild).unwrap().set_transform(translation(0.0, 0.0, 3.0));

        scene.update_nodes();

        let global = |h| scene.get_node(h).unwrap().get_global_transform();
        assert_eq!(global(root), Some(translation(1.0, 0.0, 0.0)));
        assert_eq!(global(child), Some(translation(1.0, 2.0, 0.0)));
        assert_eq!(global(grandchild), Some(translation(1.0, 2.0, 3.0)));
    }

    #[test]
    fn update_node_recomputes_subtree_transforms() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let child = scene.create_node(None, root).unwrap();
        let grandchild = scene.create_node(None, child).unwrap();
        scene.get_node_mut(root).unwrap().set_transform(translation(1.0, 0.0, 0.0));
        scene.update_nodes();

        scene.get_node_mut(child).unwrap().set_transform(translation(0.0, 5.0, 0.0));
        scene.update_node(child).unwrap();

        let global = |h| scene.get_node(h).unwrap().get_global_transform();
        assert_eq!(global(child), Some(translation(1.0, 5.0, 0.0)));
        assert_eq!(global(grandchild), Some(translation(1.0, 5.0, 0.0)));
    }

    #[test]
    fn draw_items_list_nodes_with_meshes() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let a = scene.create_node(None, root).unwrap();
        let b = scene.create_node(None, a).unwrap();
        let _no_mesh = scene.create_node(None, root).unwrap();
        let mesh_a = scene.add_mesh(test_mesh(4, 6));
        let mesh_b = scene.add_mesh(test_mesh(3, 3));
        scene.node_set_mesh(a, mesh_a).unwrap();
        scene.node_set_mesh(b, mesh_b).unwrap();
        let (shader, material) = test_material(&mut scene);
        scene.node_set_material(a, material).unwrap();
        scene.node_set_material(b, material).unwrap();
        scene.get_node_mut(a).unwrap().set_transform(translation(1.0, 0.0, 0.0));
        scene.get_node_mut(b).unwrap().set_transform(translation(0.0, 1.0, 0.0));
        scene.build_missing_mesh_buffers().unwrap();
        scene.update_nodes();

        let mut items = scene.draw_items();
        items.sort_by_key(|i| i.node);
        assert_eq!(items.len(), 2);

        let buffer = scene.buffers.iter().next().unwrap().get_handle();
        let data_a = scene.get_mesh(mesh_a).unwrap().mesh_buffer_data.as_ref().unwrap();
        let data_b = scene.get_mesh(mesh_b).unwrap().mesh_buffer_data.as_ref().unwrap();
        let item = |node| *items.iter().find(|i| i.node == node).unwrap();

        assert_eq!(
            item(a),
            DrawItem {
                node: a,
                transparent: false,
                material,
                shader,
                mesh_buffer: buffer,
                model: translation(1.0, 0.0, 0.0),
                first_index: data_a.index_begin_index as u32,
                index_count: 6,
                vertex_offset: data_a.vertex_begin_index as i32,
            }
        );
        assert_eq!(
            item(b),
            DrawItem {
                node: b,
                transparent: false,
                material,
                shader,
                mesh_buffer: buffer,
                model: translation(1.0, 1.0, 0.0),
                first_index: data_b.index_begin_index as u32,
                index_count: 3,
                vertex_offset: data_b.vertex_begin_index as i32,
            }
        );
    }

    #[test]
    fn draw_items_skip_meshes_without_buffers() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let mesh = scene.add_mesh(test_mesh(3, 3));
        scene.node_set_mesh(root, mesh).unwrap();
        let (_, material) = test_material(&mut scene);
        scene.node_set_material(root, material).unwrap();
        scene.update_nodes();

        assert!(scene.draw_items().is_empty());
    }

    #[test]
    fn draw_items_skip_nodes_without_material() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let mesh = scene.add_mesh(test_mesh(3, 3));
        scene.node_set_mesh(root, mesh).unwrap();
        scene.build_missing_mesh_buffers().unwrap();
        scene.update_nodes();

        assert!(scene.draw_items().is_empty());
    }

    #[test]
    fn draw_items_skip_materials_with_removed_shader() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let mesh = scene.add_mesh(test_mesh(3, 3));
        scene.node_set_mesh(root, mesh).unwrap();
        let (shader, material) = test_material(&mut scene);
        scene.node_set_material(root, material).unwrap();
        scene.shaders.remove(shader);
        scene.build_missing_mesh_buffers().unwrap();
        scene.update_nodes();

        assert!(scene.draw_items().is_empty());
    }

    #[test]
    fn draw_items_use_the_material_shader() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let a = scene.create_node(None, root).unwrap();
        let b = scene.create_node(None, root).unwrap();
        let mesh = scene.add_mesh(test_mesh(3, 3));
        let (shader_a, material_a) = test_material(&mut scene);
        let (shader_b, material_b) = test_material(&mut scene);
        for (node, material) in [(a, material_a), (b, material_b)] {
            scene.node_set_mesh(node, mesh).unwrap();
            scene.node_set_material(node, material).unwrap();
        }
        scene.build_missing_mesh_buffers().unwrap();
        scene.update_nodes();

        let items = scene.draw_items();
        let shader_of = |node| items.iter().find(|i| i.node == node).unwrap().shader;
        assert_eq!(shader_of(a), shader_a);
        assert_eq!(shader_of(b), shader_b);
    }

    #[test]
    fn draw_items_report_transparent_nodes() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let opaque = scene.create_node(None, root).unwrap();
        let glass = scene.create_node(None, root).unwrap();
        let mesh = scene.add_mesh(test_mesh(3, 3));
        let (_, material) = test_material(&mut scene);
        for node in [opaque, glass] {
            scene.node_set_mesh(node, mesh).unwrap();
            scene.node_set_material(node, material).unwrap();
        }
        assert!(!scene.get_node(glass).unwrap().is_transparent());
        scene.get_node_mut(glass).unwrap().set_transparent(true);
        assert!(scene.get_node(glass).unwrap().is_transparent());
        scene.build_missing_mesh_buffers().unwrap();
        scene.update_nodes();

        let items = scene.draw_items();
        let item = |node| items.iter().find(|i| i.node == node).unwrap();
        assert!(!item(opaque).transparent);
        assert!(item(glass).transparent);
    }

    fn item_at(node: usize, shader: usize, transparent: bool, y: f32) -> DrawItem {
        DrawItem {
            node,
            transparent,
            material: 0,
            shader,
            mesh_buffer: 0,
            model: translation(0.0, y, 0.0),
            first_index: 0,
            index_count: 3,
            vertex_offset: 0,
        }
    }

    #[test]
    fn opaque_items_come_first_grouped_by_shader() {
        let view = <Mat4 as cgmath::SquareMatrix>::identity();
        let mut items = vec![
            item_at(0, 2, false, 0.0),
            item_at(1, 1, true, 0.0),
            item_at(2, 1, false, 0.0),
            item_at(3, 2, false, 0.0),
            item_at(4, 0, false, 0.0),
        ];
        sort_for_rendering(&mut items, &view);
        let order: Vec<usize> = items.iter().map(|i| i.node).collect();
        // Shader 0, then 1, then 2 (in their original order), then the transparent item.
        assert_eq!(order, [4, 2, 0, 3, 1]);
    }

    #[test]
    fn transparent_items_go_from_far_to_near() {
        // A camera at the origin looking along +Y, Z up: farther means larger Y.
        let view = Mat4::look_at_rh(
            crate::math::Point3::new(0.0, 0.0, 0.0),
            crate::math::Point3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        );
        let mut items = vec![
            item_at(0, 0, true, 2.0),
            item_at(1, 0, true, 9.0),
            item_at(2, 5, false, 1.0),
            item_at(3, 0, true, 5.0),
            item_at(4, 3, true, -3.0), // behind the camera: nearest
        ];
        sort_for_rendering(&mut items, &view);
        let order: Vec<usize> = items.iter().map(|i| i.node).collect();
        assert_eq!(order, [2, 1, 3, 0, 4]);
    }

    #[test]
    fn hidden_node_hides_its_subtree() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let hidden = scene.create_node(None, root).unwrap();
        let below_hidden = scene.create_node(None, hidden).unwrap();
        let shown = scene.create_node(None, root).unwrap();
        let mesh = scene.add_mesh(test_mesh(3, 3));
        let (_, material) = test_material(&mut scene);
        for node in [hidden, below_hidden, shown] {
            scene.node_set_mesh(node, mesh).unwrap();
            scene.node_set_material(node, material).unwrap();
        }
        scene.get_node_mut(hidden).unwrap().set_visible(false);
        scene.build_missing_mesh_buffers().unwrap();
        scene.update_nodes();

        let nodes: Vec<usize> = scene.draw_items().iter().map(|i| i.node).collect();
        assert_eq!(nodes, [shown]);
    }
}
