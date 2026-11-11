use crate::scene::Scene;
use anyhow::Result;
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

/// What an app built on the engine provides: its scenes, and what it does every frame and on
/// input.
///
/// Mouse positions are in pixels of the window's drawing area (physical pixels, like the
/// rendered image), from its top left corner, with Y going down.
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

    /// Called with the size of the window's drawing area in pixels: once before the first
    /// frame, and whenever it changes (not while the window is minimized).
    fn on_resized(&mut self, _width: u32, _height: u32) -> Result<()> {
        Ok(())
    }

    /// Called when the mouse pointer moves over the window, to `(x, y)`.
    fn on_mouse_moved(&mut self, _x: f32, _y: f32) -> Result<()> {
        Ok(())
    }

    /// Called when the mouse pointer leaves the window.
    fn on_mouse_left(&mut self) -> Result<()> {
        Ok(())
    }

    /// Called when a mouse button is pressed (`pressed`) or released, with the pointer at
    /// `(x, y)`.
    fn on_mouse_button(
        &mut self,
        _button: MouseButton,
        _pressed: bool,
        _x: f32,
        _y: f32,
    ) -> Result<()> {
        Ok(())
    }

    /// Called when the mouse wheel turns, with the pointer at `(x, y)`. `delta_y` is in lines,
    /// positive when the wheel turns away from the user (scrolling up); `delta_x` is positive to
    /// the right. Touchpads, which report pixels, are converted at `PIXELS_PER_WHEEL_LINE`.
    fn on_mouse_wheel(&mut self, _delta_x: f32, _delta_y: f32, _x: f32, _y: f32) -> Result<()> {
        Ok(())
    }

    /// Text for the window's title bar. `None` leaves the title the window was created with.
    /// The engine applies this at startup and after each key press or mouse click.
    fn window_title(&self) -> Option<String> {
        None
    }
}

/// How many pixels of touchpad scrolling make one line of `FeatherApp::on_mouse_wheel`.
pub const PIXELS_PER_WHEEL_LINE: f32 = 20.0;
