use anyhow::Result;
use cgmath::Deg;

use crate::feather::assetsprovider::create_assets_provider;
use crate::feather::camera::Camera;
use crate::feather::featherapp::FeatherApp;
use crate::feather::math::{Mat4, Point3, Vec3};
use crate::feather::perspectivecamera::PerspectiveCamera;
use crate::feather::scene::Scene;

pub struct TestApp {
    scene: Scene,
    camera: PerspectiveCamera,
    /// Spun in `on_update`.
    room_node: usize,
}

impl FeatherApp for TestApp {
    fn on_create(&mut self) -> Result<()> {
        log::trace!("on_create called");
        Ok(())
    }

    fn on_render(&mut self) -> Result<()> {
        log::trace!("on_render called");
        Ok(())
    }

    fn on_update(&mut self, time: f32) -> Result<()> {
        log::trace!("on_update called with time: {}", time);
        // Spin the room around the Z axis, 10 degrees per second.
        self.scene
            .get_node_mut(self.room_node)
            .unwrap()
            .set_transform(Mat4::from_angle_z(Deg(10.0 * time)));
        Ok(())
    }

    fn on_destroy(&mut self) {
        log::trace!("on_destroy called");
    }

    fn get_num_scenes_to_render(&self) -> usize {
        1
    }

    fn get_scene_to_render(&mut self, _scene_index: usize) -> &mut Scene {
        &mut self.scene
    }

    fn get_camera_to_render_scene(&mut self, _scene_index: usize) -> &mut dyn Camera {
        &mut self.camera
    }
}

impl TestApp {
    pub fn new() -> Self {
        let assets = create_assets_provider("assets").unwrap();
        let scene = Scene::load(&*assets, "viking_room.scene.json").unwrap();
        let room_node = scene.find_node("Room").unwrap();

        let mut camera = PerspectiveCamera::new();
        camera.set_fov(45.0).set_near_far(0.1, 10.0).set_view(
            Point3::new(2.0, 2.0, 2.0),
            Point3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        );

        Self {
            scene,
            camera,
            room_node,
        }
    }
}
