use super::math::Mat4;

pub trait Camera {
    fn get_projection(&self) -> Mat4;
    fn get_view(&self) -> Mat4;
}
