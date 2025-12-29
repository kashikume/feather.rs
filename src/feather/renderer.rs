use anyhow::Result;
use super::scene::Scene;
use super::camera::Camera;

pub trait Renderer {
    fn render(&mut self, scene: &mut Scene, camera: &dyn Camera, root_handle: usize) -> Result<()>;
}