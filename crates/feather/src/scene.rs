use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;

use std::collections::HashSet;

use crate::{
    animation::Animation,
    animator::Animator,
    assetsprovider::AssetsProvider,
    atlas::Atlas,
    bounds::Frustum,
    camera::Camera,
    material::Material,
    object::Object,
    shader::{Shader, ShaderUniformNames},
    shadow::ShadowLight,
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
    /// Whether the node casts a shadow (`Node::casts_shadow`) and, when culling, may be in the
    /// shadow light's view: whether to draw it into the shadow map.
    pub casts_shadow: bool,
    /// Whether the node may be in the camera's view (always, without culling): whether to draw
    /// it in the main pass.
    pub in_view: bool,
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

/// What `Scene::draw_items` leaves out: nodes outside the camera's view, and shadow casters
/// outside the shadow light's view (see `bounds`).
#[derive(Debug, Clone)]
pub struct Culling {
    /// The camera's frustum (from its projection × view).
    pub camera: Frustum,
    /// The shadow light's frustum (from `ShadowLight::view_projection`), when the scene has one.
    pub light: Option<Frustum>,
    /// Nodes found outside the camera's view, by every `draw_items` call with this culling.
    culled: Cell<usize>,
}

impl Culling {
    /// Culling for a camera's view-projection and, if any, a shadow light's.
    pub fn new(camera_view_projection: &Mat4, light_view_projection: Option<&Mat4>) -> Self {
        Self {
            camera: Frustum::from_matrix(camera_view_projection),
            light: light_view_projection.map(Frustum::from_matrix),
            culled: Cell::new(0),
        }
    }

    /// How many nodes `draw_items` found outside the camera's view with this culling (left out
    /// of the main pass; some may still be drawn into the shadow map).
    pub fn culled_count(&self) -> usize {
        self.culled.get()
    }
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
    /// The cameras; the scene is drawn through the active one (`set_active_camera`).
    pub cameras: ObjDB<Camera>,
    active_camera: Option<usize>,
    shadow_light: Option<ShadowLight>,
    needs_create_mesh_buffer: bool,
    /// Animations changing the scene; see `add_animation`.
    animations: Animator<Scene>,
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
            cameras: ObjDB::new(),
            active_camera: None,
            shadow_light: None,
            needs_create_mesh_buffer: false,
            animations: Animator::new(),
        }
    }

    /// Identifies the scene: no two scenes created by the process have the same id. The
    /// renderer uses it to notice when it is given a different scene, whose handles mean
    /// different objects.
    pub fn get_id(&self) -> u64 {
        self.id
    }

    /// Sets the light casting the scene's shadows (`None`: no shadows). See `shadow`.
    pub fn set_shadow_light(&mut self, light: Option<ShadowLight>) {
        self.shadow_light = light;
    }

    pub fn get_shadow_light(&self) -> Option<&ShadowLight> {
        self.shadow_light.as_ref()
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

    /// Removes a node and all its descendants from the scene. Their meshes and materials stay.
    /// Fails if the node does not exist.
    /// Animations targeting any of the removed nodes (`Animation::get_target`) are cancelled.
    pub fn remove_node(&mut self, node_handle: usize) -> Result<()> {
        self.disconnect_node(node_handle)?;
        let mut removed = HashSet::new();
        let mut stack = vec![node_handle];
        while let Some(handle) = stack.pop() {
            if let Some(node) = self.nodes.get(handle) {
                stack.extend(node.get_children().iter().copied());
            }
            self.nodes.remove(handle);
            removed.insert(handle);
        }
        self.animations.cancel_targeting(&removed);
        Ok(())
    }

    /// Adds an animation changing the scene and returns its handle. It runs from the next
    /// update: the engine advances the animations of every rendered scene once per frame, after
    /// `FeatherApp::on_update` and before drawing (`advance_animations_to`). Its callbacks get the
    /// scene, so they can change anything in it, including its animations. See `animation`.
    ///
    /// `name` lets `find_animation` look the animation up. If the animation has a target node,
    /// removing that node (or an ancestor) cancels it.
    pub fn add_animation(
        &mut self,
        name: Option<String>,
        animation: impl Animation<Scene> + 'static,
    ) -> usize {
        self.animations.add(name, animation)
    }

    /// Handle of the running animation named `name`.
    pub fn find_animation(&self, name: &str) -> Option<usize> {
        self.animations.find(name)
    }

    /// Whether `handle` is a running animation (not finished or cancelled). Handles of finished
    /// animations can be given to new ones.
    pub fn is_animation_running(&self, handle: usize) -> bool {
        self.animations.is_running(handle)
    }

    /// Number of running animations.
    pub fn animation_count(&self) -> usize {
        self.animations.len()
    }

    /// Ends an animation now: applies its final state, runs its end-of-animation callback and
    /// removes it (`Animation::finish`). Does nothing for handles that aren't running, and for
    /// the animation whose callback is calling this.
    pub fn finish_animation(&mut self, handle: usize) {
        let Some(serial) = self.animations.serial_of(handle) else {
            return;
        };
        if let Some(mut taken) = self.animations.take(handle, serial) {
            taken.animation.finish(self);
            self.animations.restore(taken, true);
        }
    }

    /// Removes an animation without running any of its callbacks.
    pub fn cancel_animation(&mut self, handle: usize) {
        self.animations.cancel(handle);
    }

    /// Removes every animation without running any callbacks.
    pub fn cancel_all_animations(&mut self) {
        self.animations.clear();
    }

    /// Sets the time elapsed since an animation started (`Animation::set_elapsed`).
    pub fn set_animation_elapsed(&mut self, handle: usize, time: f32) {
        self.animations.set_elapsed(handle, time);
    }

    /// Advances every animation by `dt` seconds, removing the ones that finish. Animations
    /// added meanwhile by callbacks start in the next update.
    pub fn update_animations(&mut self, dt: f32) {
        for (handle, serial) in self.animations.running() {
            if let Some(mut taken) = self.animations.take(handle, serial) {
                let finished = taken.animation.update(self, dt);
                self.animations.restore(taken, finished);
            }
        }
    }

    /// Advances every animation to `time` (seconds, on a clock that only goes forward): by the
    /// time since the previous call, or by 0 on the first call. The engine calls it every frame
    /// with the time it passes to `FeatherApp::on_update`; an app calling it first, with the same
    /// time, gets the animations applied earlier and the engine's call then changes nothing.
    pub fn advance_animations_to(&mut self, time: f32) {
        let dt = self.animations.advance_time(time);
        self.update_animations(dt);
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

    /// Removes a mesh from the scene and from every node that uses it. Its mesh buffer is
    /// removed with its last mesh, and the renderer then releases its GPU memory. Use it for
    /// meshes that are replaced while the scene is shown, like changing text. Fails if the
    /// mesh does not exist.
    pub fn remove_mesh(&mut self, mesh_handle: usize) -> Result<()> {
        let mesh = self
            .meshes
            .get(mesh_handle)
            .ok_or_else(|| anyhow::anyhow!("Mesh {mesh_handle} not found"))?;
        if let Some(data) = &mesh.mesh_buffer_data {
            let buffer_handle = data.buffer_handle;
            if let Some(buffer) = self.buffers.get_mut(buffer_handle) {
                if buffer.remove_mesh(mesh_handle) {
                    self.buffers.remove(buffer_handle);
                }
            }
        }
        self.meshes.remove(mesh_handle);
        for node in self.nodes.iter_mut() {
            if node.get_mesh() == Some(mesh_handle) {
                node.remove_mesh();
            }
        }
        Ok(())
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
        let inherited = parent
            .map(|p| p.uniforms_for_children())
            .unwrap_or_default();
        let parent_transform =
            parent.map(|p| p.get_global_transform().unwrap_or(p.get_transform()));
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
    ///
    /// With `culling`, each item says whether its mesh may be in the camera's view (`in_view`)
    /// and whether it casts a shadow that may be in the light's view (`casts_shadow`); nodes
    /// that are neither aren't listed. Nodes with `Node::set_culled(false)` are never culled.
    /// Without `culling`, every item is `in_view`.
    pub fn draw_items(&self, culling: Option<&Culling>) -> Vec<DrawItem> {
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
            let model = node.get_global_transform().unwrap_or(node.get_transform());
            let mut in_view = true;
            let mut casts_shadow = node.casts_shadow();
            if let Some(culling) = culling.filter(|_| node.is_culled()) {
                let bounds = node
                    .get_mesh()
                    .and_then(|m| self.meshes.get(m)?.get_bounds());
                if let Some(bounds) = bounds {
                    in_view = culling.camera.intersects(bounds, &model);
                    if !in_view {
                        culling.culled.set(culling.culled.get() + 1);
                    }
                    casts_shadow = casts_shadow
                        && culling.light.map_or(true, |f| f.intersects(bounds, &model));
                }
                if !in_view && !casts_shadow {
                    continue;
                }
            }
            items.push(DrawItem {
                node: handle,
                transparent: node.is_transparent(),
                casts_shadow,
                in_view,
                material: material.get_handle(),
                shader,
                mesh_buffer: data.buffer_handle,
                model,
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

    // --- cameras ---

    /// Adds a camera and returns its handle. The first camera added (more exactly: a camera
    /// added while the scene has no active camera) becomes the active camera.
    pub fn add_camera(&mut self, camera: Camera) -> usize {
        let handle = self.cameras.add(camera);
        if self.active_camera.is_none() {
            self.active_camera = Some(handle);
        }
        handle
    }

    pub fn get_camera(&self, handle: usize) -> Option<&Camera> {
        self.cameras.get(handle)
    }

    pub fn get_camera_mut(&mut self, handle: usize) -> Option<&mut Camera> {
        self.cameras.get_mut(handle)
    }

    /// Handle of the first camera named `name`.
    pub fn find_camera(&self, name: &str) -> Option<usize> {
        self.cameras.get_by_name(name).map(|c| c.get_handle())
    }

    /// Removes a camera. If it was the active camera, the remaining camera with the lowest
    /// handle becomes active, or none if no camera is left. Invalid handles are ignored.
    pub fn remove_camera(&mut self, handle: usize) {
        self.cameras.remove(handle);
        if self.active_camera == Some(handle) {
            self.active_camera = self.cameras.iter().map(|c| c.get_handle()).min();
        }
    }

    /// Makes camera `handle` the one the scene is drawn through. Fails if there is no such
    /// camera.
    pub fn set_active_camera(&mut self, handle: usize) -> Result<()> {
        if self.cameras.get(handle).is_none() {
            anyhow::bail!("Camera {handle} not found");
        }
        self.active_camera = Some(handle);
        Ok(())
    }

    /// Handle of the camera the scene is drawn through; `None` when the scene has no camera.
    pub fn active_camera_handle(&self) -> Option<usize> {
        self.active_camera
    }

    /// The camera the scene is drawn through; `None` when the scene has no camera.
    pub fn active_camera(&self) -> Option<&Camera> {
        self.cameras.get(self.active_camera?)
    }

    pub fn active_camera_mut(&mut self) -> Option<&mut Camera> {
        self.cameras.get_mut(self.active_camera?)
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
    use cgmath::SquareMatrix;

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

        assert!(scene
            .material_set_texture(material, Some(Texture::full(42)))
            .is_err());
        assert!(scene
            .material_set_texture(42, Some(Texture::full(atlas)))
            .is_err());
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
            scene.atlases.get_by_name(TEST_PNG).unwrap().get_handle(),
            atlas
        );
    }

    #[test]
    fn create_atlas_from_png_with_name() {
        let mut scene = Scene::new();
        let atlas = scene
            .create_atlas_from_png(
                &*test_resources(),
                TEST_PNG,
                None,
                Some("bricks".to_string()),
            )
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

    // --- cameras ---

    #[test]
    fn scene_starts_without_camera() {
        let mut scene = Scene::new();
        assert_eq!(scene.active_camera_handle(), None);
        assert!(scene.active_camera().is_none());
        assert!(scene.active_camera_mut().is_none());
    }

    #[test]
    fn first_camera_added_is_active() {
        let mut scene = Scene::new();
        let first = scene.add_camera(Camera::perspective().with_name("first"));
        let second = scene.add_camera(Camera::perspective().with_name("second"));
        assert_ne!(first, second);
        assert_eq!(scene.active_camera_handle(), Some(first));
        assert_eq!(
            scene.active_camera().unwrap().get_name().as_deref(),
            Some("first")
        );
        assert_eq!(scene.find_camera("second"), Some(second));
        assert_eq!(scene.find_camera("nope"), None);
    }

    #[test]
    fn set_active_camera_switches_cameras() {
        let mut scene = Scene::new();
        let first = scene.add_camera(Camera::perspective());
        let second = scene.add_camera(Camera::perspective());
        scene.set_active_camera(second).unwrap();
        assert_eq!(scene.active_camera_handle(), Some(second));

        scene.active_camera_mut().unwrap().set_fov(70.0);
        assert_eq!(scene.get_camera(second).unwrap().get_fov(), 70.0);
        assert_eq!(scene.get_camera(first).unwrap().get_fov(), 45.0);

        let error = scene.set_active_camera(99).unwrap_err().to_string();
        assert!(error.contains("Camera 99 not found"), "{error}");
        assert_eq!(scene.active_camera_handle(), Some(second));
    }

    #[test]
    fn removing_the_active_camera_activates_the_lowest_handle() {
        let mut scene = Scene::new();
        let a = scene.add_camera(Camera::perspective());
        let b = scene.add_camera(Camera::perspective());
        let c = scene.add_camera(Camera::perspective());
        scene.set_active_camera(c).unwrap();

        // Removing an inactive camera keeps the active one.
        scene.remove_camera(b);
        assert_eq!(scene.active_camera_handle(), Some(c));

        scene.remove_camera(c);
        assert_eq!(scene.active_camera_handle(), Some(a));

        scene.remove_camera(a);
        assert_eq!(scene.active_camera_handle(), None);
        assert!(scene.active_camera().is_none());

        // With no active camera, the next camera added becomes active.
        let d = scene.add_camera(Camera::perspective());
        assert_eq!(scene.active_camera_handle(), Some(d));
    }

    #[test]
    fn find_objects_by_name() {
        let mut scene = Scene::new();
        let atlas = scene.add_atlas(test_atlas(Some("atlas")));
        let shader = scene
            .create_shader(
                Some("shader".to_string()),
                VERT,
                FRAG,
                ShaderUniformNames::default(),
            )
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
    fn remove_node_removes_its_subtree() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let kept = scene.create_node(Some("kept".into()), root).unwrap();
        let node = scene.create_node(None, root).unwrap();
        let child = scene.create_node(None, node).unwrap();
        let grandchild = scene.create_node(None, child).unwrap();
        let mesh = scene.add_mesh(test_mesh(3, 3));
        scene.node_set_mesh(child, mesh).unwrap();

        scene.remove_node(node).unwrap();
        for removed in [node, child, grandchild] {
            assert!(scene.get_node(removed).is_none());
        }
        assert_eq!(
            scene
                .get_node(root)
                .unwrap()
                .get_children()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![kept]
        );
        assert!(scene.get_mesh(mesh).is_some());
        assert!(scene.remove_node(node).is_err());
    }

    #[test]
    fn remove_mesh_removes_it_from_nodes_and_its_buffer() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let node = scene.create_node(None, root).unwrap();
        let m0 = scene.add_mesh(test_mesh(3, 3));
        let m1 = scene.add_mesh(test_mesh(4, 6));
        scene.node_set_mesh(node, m0).unwrap();
        scene.build_missing_mesh_buffers().unwrap();
        let buffer = scene.buffers.iter().next().unwrap().get_handle();

        scene.remove_mesh(m0).unwrap();
        assert!(scene.get_mesh(m0).is_none());
        assert_eq!(scene.get_node(node).unwrap().get_mesh(), None);
        // The other mesh keeps the buffer, and its place in it.
        let data = scene
            .get_mesh(m1)
            .unwrap()
            .mesh_buffer_data
            .as_ref()
            .unwrap();
        assert_eq!((data.buffer_handle, data.vertex_begin_index), (buffer, 3));
        assert_eq!(
            scene.buffers.get(buffer).unwrap().get_mesh_handles(),
            vec![m1]
        );

        // The last mesh takes the buffer with it.
        scene.remove_mesh(m1).unwrap();
        assert_eq!(scene.buffers.iter().count(), 0);
        assert!(scene.remove_mesh(m1).is_err());
    }

    #[test]
    fn a_new_buffer_reusing_a_removed_buffers_handle_has_another_id() {
        let mut scene = Scene::new();
        let m0 = scene.add_mesh(test_mesh(3, 3));
        scene.build_missing_mesh_buffers().unwrap();
        let old = scene.buffers.iter().next().unwrap();
        let (old_handle, old_id) = (old.get_handle(), old.get_id());

        scene.remove_mesh(m0).unwrap();
        let m1 = scene.add_mesh(test_mesh(5, 6));
        scene.build_missing_mesh_buffers().unwrap();
        let new = scene.buffers.iter().next().unwrap();
        assert_eq!(new.get_handle(), old_handle);
        assert_ne!(new.get_id(), old_id);
        assert_eq!(new.get_mesh_handles(), vec![m1]);
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
        let shader = scene
            .create_shader(None, VERT, FRAG, ShaderUniformNames::default())
            .unwrap();
        let material = scene.create_material(None, shader).unwrap();
        (shader, material)
    }

    #[test]
    fn update_nodes_computes_global_transforms() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let child = scene.create_node(None, root).unwrap();
        let grandchild = scene.create_node(None, child).unwrap();
        scene
            .get_node_mut(root)
            .unwrap()
            .set_transform(translation(1.0, 0.0, 0.0));
        scene
            .get_node_mut(child)
            .unwrap()
            .set_transform(translation(0.0, 2.0, 0.0));
        scene
            .get_node_mut(grandchild)
            .unwrap()
            .set_transform(translation(0.0, 0.0, 3.0));

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
        scene
            .get_node_mut(root)
            .unwrap()
            .set_transform(translation(1.0, 0.0, 0.0));
        scene.update_nodes();

        scene
            .get_node_mut(child)
            .unwrap()
            .set_transform(translation(0.0, 5.0, 0.0));
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
        scene
            .get_node_mut(a)
            .unwrap()
            .set_transform(translation(1.0, 0.0, 0.0));
        scene
            .get_node_mut(b)
            .unwrap()
            .set_transform(translation(0.0, 1.0, 0.0));
        scene.build_missing_mesh_buffers().unwrap();
        scene.update_nodes();

        let mut items = scene.draw_items(None);
        items.sort_by_key(|i| i.node);
        assert_eq!(items.len(), 2);

        let buffer = scene.buffers.iter().next().unwrap().get_handle();
        let data_a = scene
            .get_mesh(mesh_a)
            .unwrap()
            .mesh_buffer_data
            .as_ref()
            .unwrap();
        let data_b = scene
            .get_mesh(mesh_b)
            .unwrap()
            .mesh_buffer_data
            .as_ref()
            .unwrap();
        let item = |node| *items.iter().find(|i| i.node == node).unwrap();

        assert_eq!(
            item(a),
            DrawItem {
                node: a,
                transparent: false,
                casts_shadow: false,
                in_view: true,
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
                casts_shadow: false,
                in_view: true,
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

        assert!(scene.draw_items(None).is_empty());
    }

    #[test]
    fn draw_items_skip_nodes_without_material() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let mesh = scene.add_mesh(test_mesh(3, 3));
        scene.node_set_mesh(root, mesh).unwrap();
        scene.build_missing_mesh_buffers().unwrap();
        scene.update_nodes();

        assert!(scene.draw_items(None).is_empty());
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

        assert!(scene.draw_items(None).is_empty());
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

        let items = scene.draw_items(None);
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

        let items = scene.draw_items(None);
        let item = |node| items.iter().find(|i| i.node == node).unwrap();
        assert!(!item(opaque).transparent);
        assert!(item(glass).transparent);
    }

    #[test]
    fn draw_items_say_which_nodes_cast_shadows() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let mesh = scene.add_mesh(test_mesh(3, 3));
        let (_, material) = test_material(&mut scene);
        let caster = scene.create_node(None, root).unwrap();
        let other = scene.create_node(None, root).unwrap();
        for node in [caster, other] {
            scene.node_set_mesh(node, mesh).unwrap();
            scene.node_set_material(node, material).unwrap();
        }
        scene.get_node_mut(caster).unwrap().set_casts_shadow(true);
        scene.build_missing_mesh_buffers().unwrap();
        scene.update_nodes();

        let items = scene.draw_items(None);
        let item = |node| items.iter().find(|i| i.node == node).unwrap();
        assert!(item(caster).casts_shadow);
        assert!(!item(other).casts_shadow);

        assert!(scene.get_shadow_light().is_none());
        let light = crate::shadow::ShadowLight::from_matrix(translation(1.0, 0.0, 0.0));
        scene.set_shadow_light(Some(light));
        assert_eq!(scene.get_shadow_light(), Some(&light));
    }

    #[test]
    fn draw_items_cull_by_camera_and_light() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        // Vertices at x = 0, 1, 2.
        let mesh = scene.add_mesh(test_mesh(3, 3));
        let (_, material) = test_material(&mut scene);
        let node = |scene: &mut Scene, x: f32, casts_shadow: bool| {
            let node = scene.create_node(None, root).unwrap();
            scene.node_set_mesh(node, mesh).unwrap();
            scene.node_set_material(node, material).unwrap();
            let n = scene.get_node_mut(node).unwrap();
            n.set_transform(translation(x, 0.0, 0.5));
            n.set_casts_shadow(casts_shadow);
            node
        };
        // The camera sees x in -1..1, the light x in 99..101 (y -1..1, z 0..1 for both).
        let seen = node(&mut scene, 0.0, false);
        let seen_caster = node(&mut scene, 0.0, true);
        let lit_caster = node(&mut scene, 99.5, true);
        let lit_not_caster = node(&mut scene, 99.5, false);
        let neither = node(&mut scene, 50.0, true);
        let unculled = node(&mut scene, 50.0, true);
        scene.get_node_mut(unculled).unwrap().set_culled(false);
        scene.build_missing_mesh_buffers().unwrap();
        scene.update_nodes();

        let light = translation(-100.0, 0.0, 0.0);
        let culling = Culling::new(&Mat4::identity(), Some(&light));
        let items = scene.draw_items(Some(&culling));
        let item = |node| items.iter().find(|i: &&DrawItem| i.node == node);
        let flags = |node| item(node).map(|i| (i.in_view, i.casts_shadow));
        assert_eq!(culling.culled_count(), 3);
        assert_eq!(flags(seen), Some((true, false)));
        assert_eq!(flags(seen_caster), Some((true, false)));
        assert_eq!(flags(lit_caster), Some((false, true)));
        assert_eq!(flags(lit_not_caster), None);
        assert_eq!(flags(neither), None);
        assert_eq!(flags(unculled), Some((true, true)));

        // Without a shadow light, casters keep casting wherever they are.
        let culling = Culling::new(&Mat4::identity(), None);
        let items = scene.draw_items(Some(&culling));
        let flags = |node| {
            items
                .iter()
                .find(|i| i.node == node)
                .map(|i| (i.in_view, i.casts_shadow))
        };
        assert_eq!(flags(seen), Some((true, false)));
        assert_eq!(flags(neither), Some((false, true)));
        assert_eq!(flags(lit_not_caster), None);

        // Without culling, everything is listed and in view.
        let items = scene.draw_items(None);
        assert_eq!(items.len(), 6);
        assert!(items.iter().all(|i| i.in_view));
    }

    #[test]
    fn draw_items_culling_keeps_hidden_nodes_hidden() {
        let mut scene = Scene::new();
        let root = scene.create_root_node(None);
        let mesh = scene.add_mesh(test_mesh(3, 3));
        let (_, material) = test_material(&mut scene);
        let node = scene.create_node(None, root).unwrap();
        scene.node_set_mesh(node, mesh).unwrap();
        scene.node_set_material(node, material).unwrap();
        let n = scene.get_node_mut(node).unwrap();
        n.set_transform(translation(0.0, 0.0, 0.5));
        n.set_culled(false);
        n.set_visible(false);
        scene.build_missing_mesh_buffers().unwrap();
        scene.update_nodes();

        let culling = Culling::new(&Mat4::identity(), None);
        assert!(scene.draw_items(Some(&culling)).is_empty());
    }

    fn item_at(node: usize, shader: usize, transparent: bool, y: f32) -> DrawItem {
        DrawItem {
            node,
            transparent,
            casts_shadow: false,
            in_view: true,
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

        let nodes: Vec<usize> = scene.draw_items(None).iter().map(|i| i.node).collect();
        assert_eq!(nodes, [shown]);
    }

    mod animations {
        use std::ops::ControlFlow;
        use std::sync::{Arc, Mutex};

        use super::*;
        use crate::animation::{PerpetualAnimation, SlideAnimation};

        /// A slide moving `node` along X from 0 to `distance`.
        fn move_x(node: usize, duration: f32, distance: f32) -> SlideAnimation<Scene> {
            SlideAnimation::new(duration, move |scene: &mut Scene, p| {
                if let Some(n) = scene.get_node_mut(node) {
                    n.set_transform(Mat4::from_translation(Vec3::new(distance * p, 0.0, 0.0)));
                }
            })
            .target(node)
        }

        fn x_of(scene: &Scene, node: usize) -> f32 {
            scene.get_node(node).unwrap().get_transform().w.x
        }

        type Log = Arc<Mutex<Vec<String>>>;

        /// Events recorded by callbacks, shared with the test.
        fn log() -> Log {
            Arc::new(Mutex::new(Vec::new()))
        }

        fn events(log: &Log) -> Vec<String> {
            std::mem::take(&mut *log.lock().unwrap())
        }

        #[test]
        fn slide_moves_a_node_and_finishes() {
            let mut scene = Scene::new();
            let root = scene.create_root_node(None);
            let node = scene.create_node(None, root).unwrap();
            let a = scene.add_animation(Some("move".into()), move_x(node, 2.0, 4.0));
            assert_eq!(scene.find_animation("move"), Some(a));
            assert_eq!(scene.animation_count(), 1);

            scene.update_animations(1.0);
            assert_eq!(x_of(&scene, node), 2.0);
            assert!(scene.is_animation_running(a));
            scene.update_animations(1.0);
            assert_eq!(x_of(&scene, node), 4.0);
            assert!(!scene.is_animation_running(a));
            assert_eq!(scene.find_animation("move"), None);
            assert_eq!(scene.animation_count(), 0);
        }

        #[test]
        fn advance_animations_to_uses_time_differences() {
            let mut scene = Scene::new();
            let root = scene.create_root_node(None);
            let node = scene.create_node(None, root).unwrap();
            scene.add_animation(None, move_x(node, 4.0, 4.0));
            scene.advance_animations_to(100.0);
            assert_eq!(x_of(&scene, node), 0.0);
            scene.advance_animations_to(101.0);
            assert_eq!(x_of(&scene, node), 1.0);
            // Same time again (the app first, then the engine): no change.
            scene.advance_animations_to(101.0);
            assert_eq!(x_of(&scene, node), 1.0);
            scene.advance_animations_to(103.0);
            assert_eq!(x_of(&scene, node), 3.0);
        }

        #[test]
        fn callbacks_can_add_animations_which_start_in_the_next_update() {
            let mut scene = Scene::new();
            let root = scene.create_root_node(None);
            let node = scene.create_node(None, root).unwrap();
            // Out to 2 in one second, then back from 2 to 0 in one second.
            scene.add_animation(
                None,
                move_x(node, 1.0, 2.0).on_finish(move |scene: &mut Scene| {
                    scene.add_animation(
                        Some("back".into()),
                        SlideAnimation::new(1.0, move |scene: &mut Scene, p| {
                            let x = 2.0 * (1.0 - p);
                            scene
                                .get_node_mut(node)
                                .unwrap()
                                .set_transform(Mat4::from_translation(Vec3::new(x, 0.0, 0.0)));
                        }),
                    );
                }),
            );
            scene.update_animations(1.0);
            assert_eq!(x_of(&scene, node), 2.0);
            let back = scene.find_animation("back").unwrap();
            assert!(scene.is_animation_running(back));
            scene.update_animations(0.5);
            assert_eq!(x_of(&scene, node), 1.0);
            scene.update_animations(0.5);
            assert_eq!(x_of(&scene, node), 0.0);
            assert_eq!(scene.animation_count(), 0);
        }

        #[test]
        fn callbacks_can_cancel_and_finish_other_animations() {
            let mut scene = Scene::new();
            let root = scene.create_root_node(None);
            let a = scene.create_node(None, root).unwrap();
            let b = scene.create_node(None, root).unwrap();
            let c = scene.create_node(None, root).unwrap();
            let slide_b = scene.add_animation(None, move_x(b, 10.0, 10.0));
            let slide_c = scene.add_animation(None, move_x(c, 10.0, 10.0));
            scene.add_animation(
                None,
                move_x(a, 1.0, 1.0).on_finish(move |scene: &mut Scene| {
                    scene.cancel_animation(slide_b);
                    scene.finish_animation(slide_c);
                }),
            );
            scene.update_animations(1.0);
            assert_eq!(scene.animation_count(), 0);
            // b was moved once (it was added before a, so it ran first), then cancelled; c was
            // moved to its end.
            assert_eq!(x_of(&scene, b), 1.0);
            assert_eq!(x_of(&scene, c), 10.0);
            scene.update_animations(1.0);
            assert_eq!(x_of(&scene, b), 1.0);
        }

        #[test]
        fn an_animation_can_cancel_itself() {
            let mut scene = Scene::new();
            let log = log();
            let seen = log.clone();
            scene.add_animation(
                Some("self".into()),
                PerpetualAnimation::new(move |scene: &mut Scene, _, _| {
                    seen.lock().unwrap().push("step".into());
                    let me = scene.find_animation("self").unwrap();
                    scene.cancel_animation(me);
                    ControlFlow::Continue(())
                }),
            );
            scene.update_animations(1.0);
            scene.update_animations(1.0);
            assert_eq!(events(&log), ["step"]);
            assert_eq!(scene.animation_count(), 0);
        }

        #[test]
        fn a_handle_reused_during_an_update_keeps_the_new_animation() {
            let mut scene = Scene::new();
            let log = log();
            let seen = log.clone();
            // Cancels itself and adds a new animation, which gets its handle.
            scene.add_animation(
                Some("old".into()),
                PerpetualAnimation::new(move |scene: &mut Scene, _, _| {
                    let me = scene.find_animation("old").unwrap();
                    scene.cancel_animation(me);
                    let seen = seen.clone();
                    let new = scene.add_animation(
                        None,
                        PerpetualAnimation::new(move |_: &mut Scene, _, elapsed| {
                            seen.lock().unwrap().push(format!("new {elapsed}"));
                            ControlFlow::Continue(())
                        }),
                    );
                    assert_eq!(new, me);
                    ControlFlow::Continue(())
                }),
            );
            scene.update_animations(1.0);
            // The new animation didn't run in the update that added it ...
            assert!(events(&log).is_empty());
            assert_eq!(scene.animation_count(), 1);
            // ... and wasn't replaced by the old one when that one was put back.
            scene.update_animations(1.0);
            assert_eq!(events(&log), ["new 1"]);
        }

        #[test]
        fn finishing_an_animation_runs_its_end() {
            let mut scene = Scene::new();
            let root = scene.create_root_node(None);
            let node = scene.create_node(None, root).unwrap();
            let log = log();
            let seen = log.clone();
            let a = scene.add_animation(
                None,
                move_x(node, 10.0, 5.0)
                    .on_finish(move |_: &mut Scene| seen.lock().unwrap().push("done".into())),
            );
            scene.update_animations(1.0);
            scene.finish_animation(a);
            assert_eq!(x_of(&scene, node), 5.0);
            assert_eq!(events(&log), ["done"]);
            assert!(!scene.is_animation_running(a));
            scene.finish_animation(a);
            assert!(events(&log).is_empty());
        }

        #[test]
        fn set_elapsed_and_cancel_all() {
            let mut scene = Scene::new();
            let root = scene.create_root_node(None);
            let node = scene.create_node(None, root).unwrap();
            let a = scene.add_animation(None, move_x(node, 4.0, 4.0));
            scene.set_animation_elapsed(a, 3.0);
            scene.update_animations(0.0);
            assert_eq!(x_of(&scene, node), 3.0);
            scene.add_animation(None, move_x(node, 4.0, 4.0));
            scene.cancel_all_animations();
            assert_eq!(scene.animation_count(), 0);
        }

        #[test]
        fn removing_a_target_node_cancels_its_animations() {
            let mut scene = Scene::new();
            let root = scene.create_root_node(None);
            let parent = scene.create_node(None, root).unwrap();
            let child = scene.create_node(None, parent).unwrap();
            let other = scene.create_node(None, root).unwrap();
            let log = log();
            let seen = log.clone();
            let on_child = scene.add_animation(
                None,
                move_x(child, 1.0, 1.0)
                    .on_finish(move |_: &mut Scene| seen.lock().unwrap().push("done".into())),
            );
            let on_other = scene.add_animation(None, move_x(other, 1.0, 1.0));
            scene.remove_node(parent).unwrap();
            assert!(!scene.is_animation_running(on_child));
            assert!(scene.is_animation_running(on_other));
            scene.update_animations(1.0);
            assert!(events(&log).is_empty());
            assert_eq!(x_of(&scene, other), 1.0);
        }

        #[test]
        fn an_animation_can_remove_its_own_target() {
            let mut scene = Scene::new();
            let root = scene.create_root_node(None);
            let node = scene.create_node(None, root).unwrap();
            let a = scene.add_animation(
                None,
                move_x(node, 1.0, 1.0).on_finish(move |scene: &mut Scene| {
                    scene.remove_node(node).unwrap();
                }),
            );
            scene.update_animations(0.5);
            scene.update_animations(0.5);
            assert!(scene.get_node(node).is_none());
            assert!(!scene.is_animation_running(a));
            assert_eq!(scene.animation_count(), 0);
        }

        #[test]
        fn scenes_with_animations_can_go_to_other_threads() {
            fn assert_send_sync<T: Send + Sync>() {}
            assert_send_sync::<Scene>();
            assert_send_sync::<Animator<Scene>>();

            let mut scene = Scene::new();
            let root = scene.create_root_node(None);
            let node = scene.create_node(None, root).unwrap();
            scene.add_animation(None, move_x(node, 2.0, 2.0));
            let scene = std::thread::spawn(move || {
                let mut scene = scene;
                scene.update_animations(1.0);
                scene
            })
            .join()
            .unwrap();
            assert_eq!(x_of(&scene, node), 1.0);
            assert_eq!(scene.animation_count(), 1);
        }
    }
}
