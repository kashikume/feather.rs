use super::object::Object;
use super::texture::Texture;
use super::uniforms::Uniforms;

/// How to draw a surface: a shader, the texture it samples and the uniform values passed to
/// it.
///
/// Every sampler of the shader samples the texture's atlas, and the shader's `umin`, `umax`,
/// `vmin` and `vmax` uniforms (see `ShaderUniformNames`) get the texture's rectangle.
pub struct Material {
    name: Option<String>,
    handle: usize,
    /// Handle of the shader in `Scene::shaders`.
    shader: usize,
    texture: Option<Texture>,
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
            texture: None,
            uniforms: Uniforms::new(),
        }
    }

    pub fn get_shader(&self) -> usize {
        self.shader
    }

    pub fn set_shader(&mut self, shader: usize) {
        self.shader = shader;
    }

    pub fn get_texture(&self) -> Option<&Texture> {
        self.texture.as_ref()
    }

    /// Sets the texture the material's shader samples (`None` for no texture). Use
    /// `Scene::material_set_texture` to also check that the texture's atlas exists.
    pub fn set_texture(&mut self, texture: Option<Texture>) {
        self.texture = texture;
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
