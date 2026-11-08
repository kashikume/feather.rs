use super::object::Object;

pub struct Atlas {
    name: Option<String>,
    handle: usize,
    /// Handles (in `Scene::textures`) of all textures in this atlas, including the full texture.
    textures: Vec<usize>,
    /// Handle of the texture covering the whole atlas, if there is one.
    full_texture: Option<usize>,
}

impl Object for Atlas {
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

impl Atlas {
    pub fn new(name: Option<String>) -> Self {
        Self {
            name,
            handle: usize::MAX,
            textures: Vec::new(),
            full_texture: None,
        }
    }

    /// Adds a texture handle. Adding the same handle twice keeps one copy.
    pub fn add_texture(&mut self, texture: usize) {
        if !self.textures.contains(&texture) {
            self.textures.push(texture);
        }
    }

    /// Removes a texture handle. If it was the full texture, the atlas no longer has one.
    pub fn remove_texture(&mut self, texture: usize) {
        self.textures.retain(|&t| t != texture);
        if self.full_texture == Some(texture) {
            self.full_texture = None;
        }
    }

    pub fn get_textures(&self) -> &[usize] {
        &self.textures
    }

    /// Sets the texture covering the whole atlas. It is also added to the texture list.
    pub fn set_full_texture(&mut self, texture: usize) {
        self.add_texture(texture);
        self.full_texture = Some(texture);
    }

    pub fn get_full_texture(&self) -> Option<usize> {
        self.full_texture
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_atlas_is_empty() {
        let atlas = Atlas::new(Some("atlas".to_string()));
        assert_eq!(atlas.get_name(), Some("atlas".to_string()));
        assert_eq!(atlas.get_handle(), usize::MAX);
        assert!(atlas.get_textures().is_empty());
        assert_eq!(atlas.get_full_texture(), None);
    }

    #[test]
    fn add_and_remove_textures() {
        let mut atlas = Atlas::new(None);
        atlas.add_texture(1);
        atlas.add_texture(2);
        atlas.add_texture(1);
        assert_eq!(atlas.get_textures(), &[1, 2]);

        atlas.remove_texture(1);
        assert_eq!(atlas.get_textures(), &[2]);

        atlas.remove_texture(42); // missing handle is ignored
        assert_eq!(atlas.get_textures(), &[2]);
    }

    #[test]
    fn full_texture_is_in_texture_list() {
        let mut atlas = Atlas::new(None);
        atlas.add_texture(1);
        atlas.set_full_texture(2);

        assert_eq!(atlas.get_full_texture(), Some(2));
        assert_eq!(atlas.get_textures(), &[1, 2]);
    }

    #[test]
    fn set_full_texture_to_existing_texture_does_not_duplicate() {
        let mut atlas = Atlas::new(None);
        atlas.add_texture(1);
        atlas.set_full_texture(1);
        assert_eq!(atlas.get_textures(), &[1]);
    }

    #[test]
    fn replacing_full_texture_keeps_old_one_in_list() {
        let mut atlas = Atlas::new(None);
        atlas.set_full_texture(1);
        atlas.set_full_texture(2);

        assert_eq!(atlas.get_full_texture(), Some(2));
        assert_eq!(atlas.get_textures(), &[1, 2]);
    }

    #[test]
    fn removing_full_texture_clears_it() {
        let mut atlas = Atlas::new(None);
        atlas.set_full_texture(1);
        atlas.remove_texture(1);

        assert_eq!(atlas.get_full_texture(), None);
        assert!(atlas.get_textures().is_empty());
    }
}
