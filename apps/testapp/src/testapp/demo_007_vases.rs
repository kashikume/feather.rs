use anyhow::{anyhow, Result};
use cgmath::Deg;

use feather::assetsprovider::AssetsProvider;
use feather::camera::Camera;
use feather::math::{Mat4, Point3, Vec3};
use feather::perspectivecamera::PerspectiveCamera;
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
    camera: PerspectiveCamera,
    turntable_node: usize,
}

impl VasesDemo {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        let scene = Scene::load(assets, "demo_007_vases_scene.json")?;
        let turntable_node = scene
            .find_node("Turntable")
            .ok_or_else(|| anyhow!("the scene has no node `Turntable`"))?;

        // In front of the floor and above it, looking at its middle.
        let mut camera = PerspectiveCamera::new();
        camera.set_fov(45.0).set_near_far(0.1, 50.0).set_view(
            Point3::new(0.0, -10.0, 5.0),
            Point3::new(0.0, 0.0, 0.6),
            Vec3::new(0.0, 0.0, 1.0),
        );

        Ok(Self {
            scene,
            camera,
            turntable_node,
        })
    }
}

impl Demo for VasesDemo {
    fn scene(&mut self) -> &mut Scene {
        &mut self.scene
    }

    fn camera(&mut self) -> &mut dyn Camera {
        &mut self.camera
    }

    fn update(&mut self, time: f32) -> Result<()> {
        self.scene
            .get_node_mut(self.turntable_node)
            .unwrap()
            .set_transform(Mat4::from_angle_z(Deg(TURN_SPEED * time)));
        Ok(())
    }
}
