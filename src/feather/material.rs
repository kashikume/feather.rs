use super::shader::Shader;
use super::texture::Texture;

pub struct Material {
    shader: Shader,
    texture: Option<Texture>,
}
