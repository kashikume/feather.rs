use crate::scene::Scene;
use anyhow::Result;
use winit::keyboard::KeyCode;

pub trait FeatherApp {
    fn on_create(&mut self) -> Result<()>;
    fn on_render(&mut self) -> Result<()>;
    fn on_update(&mut self, time: f32) -> Result<()>;
    fn on_destroy(&mut self);

    fn get_num_scenes_to_render(&self) -> usize;
    /// Scene `scene_index` (`0..get_num_scenes_to_render()`), drawn through its active camera
    /// over the scenes before it.
    fn get_scene_to_render(&mut self, scene_index: usize) -> &mut Scene;

    /// Called when a key is pressed (not for auto-repeat). `key` is the physical key, so it
    /// doesn't depend on the keyboard layout.
    fn on_key_pressed(&mut self, _key: KeyCode) -> Result<()> {
        Ok(())
    }

    /// Text for the window's title bar. `None` leaves the title the window was created with.
    /// The engine applies this at startup and after each key press.
    fn window_title(&self) -> Option<String> {
        None
    }
}
