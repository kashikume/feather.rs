use anyhow::{anyhow, Result};
use cgmath::Deg;

use feather::assetsprovider::AssetsProvider;
use feather::math::Mat4;
use feather::scene::Scene;

use super::testapp::Demo;

/// Degrees the turntable turns per second.
const TURN_SPEED: f32 = 15.0;

/// Demo 7: 12 vases on a floor plate, slowly turning.
///
/// Everything is in `demo_007_vases_scene.json`: three vase shapes loaded from OBJ files (made
/// by spinning a profile with thick walls around the Z axis), the floor, and 12 vases of those
/// shapes in different sizes, each with its own `color` uniform. Five of them are transparent
/// nodes with the see-through `Glass` material; the engine draws those after the opaque ones,
/// the farthest first. This demo only turns the `Turntable` node holding the floor and vases.
pub struct VasesDemo {
    scene: Scene,
    turntable_node: usize,
}

impl VasesDemo {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        let scene = Scene::load(assets, "demo_007_vases_scene.json")?;
        let turntable_node = scene
            .find_node("Turntable")
            .ok_or_else(|| anyhow!("the scene has no node `Turntable`"))?;

        // The scene file's camera: in front of the floor and above it, looking at its middle.
        Ok(Self {
            scene,
            turntable_node,
        })
    }
}

impl Demo for VasesDemo {
    fn scene(&mut self) -> &mut Scene {
        &mut self.scene
    }

    fn update(&mut self, time: f32) -> Result<()> {
        self.scene
            .get_node_mut(self.turntable_node)
            .unwrap()
            .set_transform(Mat4::from_angle_z(Deg(TURN_SPEED * time)));
        Ok(())
    }
}
