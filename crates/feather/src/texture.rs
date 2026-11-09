use super::math::Vec4;
use super::object::Object;

/// A rectangle of an atlas, used as a texture. Coordinates are in UV space (0..1 across the
/// atlas).
///
/// Shaders sample the whole atlas; to use only the texture's rectangle, a shader declares a
/// `vec4 <sampler>Rect` uniform next to its sampler (e.g. `texSamplerRect` for `texSampler`),
/// which the engine fills with `get_uv_rect`, and maps its coordinates with
/// `rect.xy + uv * rect.zw`.
pub struct Texture {
    name: Option<String>,
    handle: usize,
    /// Handle of the atlas in `Scene::atlases`.
    atlas: usize,
    u_min: f32,
    v_min: f32,
    u_range: f32,
    v_range: f32,
}

impl Object for Texture {
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

impl Texture {
    /// A texture covering the rectangle that starts at (`u_min`, `v_min`) and is `u_range`
    /// wide and `v_range` high, in UV space.
    pub fn new(
        name: Option<String>,
        atlas: usize,
        u_min: f32,
        v_min: f32,
        u_range: f32,
        v_range: f32,
    ) -> Self {
        Self {
            name,
            handle: usize::MAX,
            atlas,
            u_min,
            v_min,
            u_range,
            v_range,
        }
    }

    /// A texture covering the whole atlas.
    pub fn full_atlas(name: Option<String>, atlas: usize) -> Self {
        Self::new(name, atlas, 0.0, 0.0, 1.0, 1.0)
    }

    pub fn get_atlas(&self) -> usize {
        self.atlas
    }

    /// The texture's rectangle as (u_min, v_min, u_range, v_range).
    pub fn get_uv_rect(&self) -> Vec4 {
        Vec4::new(self.u_min, self.v_min, self.u_range, self.v_range)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_atlas_covers_everything() {
        let texture = Texture::full_atlas(Some("all".to_string()), 3);
        assert_eq!(texture.get_name(), Some("all".to_string()));
        assert_eq!(texture.get_atlas(), 3);
        assert_eq!(texture.get_handle(), usize::MAX);
        assert_eq!(texture.get_uv_rect(), Vec4::new(0.0, 0.0, 1.0, 1.0));
    }

    #[test]
    fn new_keeps_rectangle() {
        let texture = Texture::new(None, 1, 0.5, 0.25, 0.5, 0.125);
        assert_eq!(texture.get_atlas(), 1);
        assert_eq!(texture.get_uv_rect(), Vec4::new(0.5, 0.25, 0.5, 0.125));
    }
}
