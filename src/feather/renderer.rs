use super::camera::Camera;
use super::scene::Scene;
use anyhow::Result;

pub trait Renderer {
    fn prepare_scene(&self, scene: &mut Scene) -> Result<()>;
    fn render(&mut self, scene: &mut Scene, camera: &dyn Camera, root_handle: usize) -> Result<()>;
}
