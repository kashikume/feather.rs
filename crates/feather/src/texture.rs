/// A rectangle of an atlas, used as a texture. Coordinates are in UV space (0..1 across the
/// atlas, `v` = 0 at the top row of the image).
///
/// A texture is a plain value: get one from its atlas (`Atlas::get_texture`,
/// `Atlas::get_full_texture`) and store it where it is used, e.g. in a `Material`.
///
/// Shaders sample the whole atlas. To use only the texture's rectangle, a shader declares
/// `float` uniforms for its edges and names them in its `ShaderUniformNames` (`umin`, `umax`,
/// `vmin`, `vmax`); the engine fills them from the material's texture, and the shader maps its
/// coordinates with `mix(vec2(umin, vmin), vec2(umax, vmax), uv)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Texture {
    /// Handle of the atlas in `Scene::atlases`.
    atlas: usize,
    u_min: f32,
    u_max: f32,
    v_min: f32,
    v_max: f32,
}

impl Texture {
    /// A texture of atlas `atlas` covering `u_min..u_max` and `v_min..v_max` in UV space.
    pub fn new(atlas: usize, u_min: f32, u_max: f32, v_min: f32, v_max: f32) -> Self {
        Self {
            atlas,
            u_min,
            u_max,
            v_min,
            v_max,
        }
    }

    /// A texture covering the whole atlas.
    pub fn full(atlas: usize) -> Self {
        Self::new(atlas, 0.0, 1.0, 0.0, 1.0)
    }

    /// Handle of the atlas (in `Scene::atlases`).
    pub fn get_atlas(&self) -> usize {
        self.atlas
    }

    pub fn get_u_min(&self) -> f32 {
        self.u_min
    }

    pub fn get_u_max(&self) -> f32 {
        self.u_max
    }

    pub fn get_v_min(&self) -> f32 {
        self.v_min
    }

    pub fn get_v_max(&self) -> f32 {
        self.v_max
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_covers_everything() {
        let texture = Texture::full(3);
        assert_eq!(texture.get_atlas(), 3);
        assert_eq!(
            (
                texture.get_u_min(),
                texture.get_u_max(),
                texture.get_v_min(),
                texture.get_v_max()
            ),
            (0.0, 1.0, 0.0, 1.0)
        );
    }

    #[test]
    fn new_keeps_rectangle() {
        let texture = Texture::new(1, 0.5, 1.0, 0.25, 0.375);
        assert_eq!(texture.get_atlas(), 1);
        assert_eq!(
            (
                texture.get_u_min(),
                texture.get_u_max(),
                texture.get_v_min(),
                texture.get_v_max()
            ),
            (0.5, 1.0, 0.25, 0.375)
        );
    }
}
