use anyhow::{anyhow, Result};
use cgmath::Deg;

use feather::assetsprovider::AssetsProvider;
use feather::math::Mat4;
use feather::scene::Scene;

use super::testapp::Demo;

/// Demo 1: a textured cuboid, loaded from `demo_001_cuboid_scene.json`, tumbling.
///
/// The scene has node `Rotation`, which this demo spins, holding node `Cube`, which has the mesh
/// and the scale (set in the scene file).
pub struct CuboidDemo {
    scene: Scene,
    rotation_node: usize,
}

impl CuboidDemo {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        let scene = Scene::load(assets, "demo_001_cuboid_scene.json")?;
        let rotation_node = scene
            .find_node("Rotation")
            .ok_or_else(|| anyhow!("the scene has no node `Rotation`"))?;

        Ok(Self {
            scene,
            rotation_node,
        })
    }
}

impl Demo for CuboidDemo {
    fn scene(&mut self) -> &mut Scene {
        &mut self.scene
    }

    fn update(&mut self, time: f32) -> Result<()> {
        // Tumble: around Z at 30 degrees per second, around X at 20. This sets only the
        // rotation; the child `Cube` node keeps the scale from the scene file.
        let rotation = Mat4::from_angle_z(Deg(30.0 * time)) * Mat4::from_angle_x(Deg(20.0 * time));
        self.scene
            .get_node_mut(self.rotation_node)
            .unwrap()
            .set_transform(rotation);
        Ok(())
    }
}
