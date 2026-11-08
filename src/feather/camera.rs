use super::math::Mat4;

pub trait Camera {
    fn get_projection(&self) -> Mat4;
    fn get_view(&self) -> Mat4;
    /// Called by the renderer with the size of the image being rendered.
    fn set_screen_dimention(&mut self, width: u32, height: u32);
}
