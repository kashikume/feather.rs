use super::shader::Shader;
use super::texture::Texture;
use super::uniforms::Uniforms;

pub struct Material {
    shader: Shader,
    texture: Option<Texture>,
    uniforms: Uniforms,
}

impl Material {
    pub fn uniforms(&self) -> &Uniforms {
        &self.uniforms
    }

    pub fn uniforms_mut(&mut self) -> &mut Uniforms {
        &mut self.uniforms
    }
}
