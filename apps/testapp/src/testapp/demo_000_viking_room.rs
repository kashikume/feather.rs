use anyhow::{anyhow, Result};
use cgmath::Deg;

use feather::assetsprovider::AssetsProvider;
use feather::camera::Camera;
use feather::math::{Mat4, Point3, Vec3};
use feather::perspectivecamera::PerspectiveCamera;
use feather::scene::Scene;

use super::testapp::Demo;

/// Demo 0: the viking room, loaded from `demo_000_viking_room_scene.json`, spinning.
pub struct VikingRoomDemo {
    scene: Scene,
    camera: PerspectiveCamera,
    room_node: usize,
}

impl VikingRoomDemo {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        let scene = Scene::load(assets, "demo_000_viking_room_scene.json")?;
        let room_node = scene
            .find_node("Room")
            .ok_or_else(|| anyhow!("the scene has no node `Room`"))?;

        let mut camera = PerspectiveCamera::new();
        camera.set_fov(45.0).set_near_far(0.1, 10.0).set_view(
            Point3::new(2.0, 2.0, 2.0),
            Point3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        );

        Ok(Self {
            scene,
            camera,
            room_node,
        })
    }
}

impl Demo for VikingRoomDemo {
    fn scene(&mut self) -> &mut Scene {
        &mut self.scene
    }

    fn camera(&mut self) -> &mut dyn Camera {
        &mut self.camera
    }

    fn update(&mut self, time: f32) -> Result<()> {
        // Spin the room around the Z axis, 10 degrees per second.
        self.scene
            .get_node_mut(self.room_node)
            .unwrap()
            .set_transform(Mat4::from_angle_z(Deg(10.0 * time)));
        Ok(())
    }
}
