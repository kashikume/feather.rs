use super::object::Object;
use super::uniforms::Uniforms;

/// How to draw a surface: a shader plus the uniform values passed to it. Textures are
/// uniforms too (`UniformValue::Texture`).
pub struct Material {
    name: Option<String>,
    handle: usize,
    /// Handle of the shader in `Scene::shaders`.
    shader: usize,
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
            uniforms: Uniforms::new(),
        }
    }

    pub fn get_shader(&self) -> usize {
        self.shader
    }

    pub fn set_shader(&mut self, shader: usize) {
        self.shader = shader;
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
    use crate::feather::uniform::UniformValue;

    #[test]
    fn new_material() {
        let material = Material::new(Some("wood".to_string()), 2);
        assert_eq!(material.get_name(), Some("wood".to_string()));
        assert_eq!(material.get_handle(), usize::MAX);
        assert_eq!(material.get_shader(), 2);
        assert!(material.uniforms().is_empty());
    }

    #[test]
    fn set_shader() {
        let mut material = Material::new(None, 2);
        material.set_shader(5);
        assert_eq!(material.get_shader(), 5);
    }

    #[test]
    fn uniforms() {
        let mut material = Material::new(None, 0);
        material
            .uniforms_mut()
            .set("texSampler", UniformValue::Texture(1))
            .set("roughness", 0.5f32);

        assert_eq!(material.uniforms().len(), 2);
        assert_eq!(
            material.uniforms().get("texSampler"),
            Some(&UniformValue::Texture(1))
        );
    }
}
