use super::object::Object;
use super::texture::Texture;
use super::uniforms::Uniforms;

/// Number of texture slots of a material.
pub const MAX_MATERIAL_TEXTURES: usize = 4;

/// How to draw a surface: a shader, the textures it samples and the uniform values passed to
/// it.
///
/// A material has `MAX_MATERIAL_TEXTURES` texture slots, each empty or holding a texture. The
/// shader's texture samplers (`ShaderUniformNames::texture_samplers`) sample them in order:
/// sampler `i` samples the atlas of slot `i`, and the rectangle uniforms named for sampler `i`
/// get the rectangle of slot `i`'s texture.
pub struct Material {
    name: Option<String>,
    handle: usize,
    /// Handle of the shader in `Scene::shaders`.
    shader: usize,
    textures: [Option<Texture>; MAX_MATERIAL_TEXTURES],
    uniforms: Uniforms,
}

impl Object for Material {
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

impl Material {
    pub fn new(name: Option<String>, shader: usize) -> Self {
        Self {
            name,
            handle: usize::MAX,
            shader,
            textures: [None; MAX_MATERIAL_TEXTURES],
            uniforms: Uniforms::new(),
        }
    }

    pub fn get_shader(&self) -> usize {
        self.shader
    }

    pub fn set_shader(&mut self, shader: usize) {
        self.shader = shader;
    }

    /// The texture in slot 0.
    pub fn get_texture(&self) -> Option<&Texture> {
        self.textures[0].as_ref()
    }

    /// Sets the texture in slot 0 (`None` to empty it). Use `Scene::material_set_texture` to
    /// also check that the texture's atlas exists.
    pub fn set_texture(&mut self, texture: Option<Texture>) {
        self.textures[0] = texture;
    }

    /// The texture in slot `slot`; `None` when the slot is empty or past the last one.
    pub fn get_texture_at(&self, slot: usize) -> Option<&Texture> {
        self.textures.get(slot)?.as_ref()
    }

    /// Sets the texture in slot `slot` (`None` to empty it). Use
    /// `Scene::material_set_texture_at` to also check the slot and the texture's atlas.
    /// Panics if `slot` is not below `MAX_MATERIAL_TEXTURES`.
    pub fn set_texture_at(&mut self, slot: usize, texture: Option<Texture>) {
        assert!(
            slot < MAX_MATERIAL_TEXTURES,
            "texture slot {slot} out of range (a material has {MAX_MATERIAL_TEXTURES})"
        );
        self.textures[slot] = texture;
    }

    /// All texture slots, in order.
    pub fn get_textures(&self) -> &[Option<Texture>; MAX_MATERIAL_TEXTURES] {
        &self.textures
    }

    pub fn uniforms(&self) -> &Uniforms {
        &self.uniforms
    }

    pub fn uniforms_mut(&mut self) -> &mut Uniforms {
        &mut self.uniforms
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::uniform::UniformValue;

    #[test]
    fn new_material() {
        let material = Material::new(Some("wood".to_string()), 2);
        assert_eq!(material.get_name(), Some("wood".to_string()));
        assert_eq!(material.get_handle(), usize::MAX);
        assert_eq!(material.get_shader(), 2);
        assert_eq!(material.get_texture(), None);
        assert!(material.uniforms().is_empty());
    }

    #[test]
    fn set_shader() {
        let mut material = Material::new(None, 2);
        material.set_shader(5);
        assert_eq!(material.get_shader(), 5);
    }

    #[test]
    fn set_texture() {
        let mut material = Material::new(None, 0);
        let texture = Texture::new(1, 0.0, 0.5, 0.5, 1.0);
        material.set_texture(Some(texture));
        assert_eq!(material.get_texture(), Some(&texture));
        material.set_texture(None);
        assert_eq!(material.get_texture(), None);
    }

    #[test]
    fn texture_slots() {
        let mut material = Material::new(None, 0);
        let color = Texture::full(1);
        let normals = Texture::full(2);
        material.set_texture(Some(color));
        material.set_texture_at(1, Some(normals));
        assert_eq!(material.get_texture_at(0), Some(&color));
        assert_eq!(material.get_texture_at(1), Some(&normals));
        assert_eq!(material.get_texture_at(2), None);
        assert_eq!(material.get_texture_at(MAX_MATERIAL_TEXTURES), None);
        assert_eq!(
            material.get_textures(),
            &[Some(color), Some(normals), None, None]
        );
        material.set_texture_at(0, None);
        assert_eq!(material.get_texture(), None);
        assert_eq!(material.get_texture_at(1), Some(&normals));
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn texture_slot_past_the_last_panics() {
        Material::new(None, 0).set_texture_at(MAX_MATERIAL_TEXTURES, None);
    }

    #[test]
    fn uniforms() {
        let mut material = Material::new(None, 0);
        material
            .uniforms_mut()
            .set("tint", UniformValue::Float(0.25))
            .set("roughness", 0.5f32);

        assert_eq!(material.uniforms().len(), 2);
        assert_eq!(
            material.uniforms().get("tint"),
            Some(&UniformValue::Float(0.25))
        );
    }
}
