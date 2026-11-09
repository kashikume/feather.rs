use crate::camera::Camera;
use crate::scene::Scene;
use anyhow::Result;
use winit::keyboard::KeyCode;

pub trait FeatherApp {
    fn on_create(&mut self) -> Result<()>;
    fn on_render(&mut self) -> Result<()>;
    fn on_update(&mut self, time: f32) -> Result<()>;
    fn on_destroy(&mut self);

    fn get_num_scenes_to_render(&self) -> usize;
    fn get_scene_to_render(&mut self, scene_index: usize) -> &mut Scene;
    fn get_camera_to_render_scene(&mut self, scene_index: usize) -> &mut dyn Camera;

    /// Called when a key is pressed (not for auto-repeat). `key` is the physical key, so it
    /// doesn't depend on the keyboard layout.
    fn on_key_pressed(&mut self, _key: KeyCode) -> Result<()> {
        Ok(())
    }
}
