use anyhow::{anyhow, Result};
use cgmath::Deg;

use feather::assetsprovider::AssetsProvider;
use feather::featherapp::SceneTextureTarget;
use feather::math::{Mat4, Point3};
use feather::scene::Scene;

use super::testapp::Demo;

/// Degrees the movie's room turns per second (as in demo 0).
const ROOM_SPEED: f32 = 10.0;
/// How far the camera sways to each side (scene units), and how fast (radians per second).
const SWAY: f32 = 3.5;
const SWAY_SPEED: f32 = 0.15;

/// Demo 23: a drive-in cinema at night, with a scene drawn into a texture.
///
/// Two scenes:
/// - the **movie**: demo 0's scene file (`demo_000_viking_room_scene.json`), its room turning.
///   It is a texture scene (`Demo::texture_scene_at`): every frame the engine draws it into the
///   drive-in's scene texture `Screen` (`"scene_texture"` in the scene file, 1280 x 544), with
///   its camera's aspect set to the texture's;
/// - the **drive-in** (`demo_023_drive_in_scene.json`), the scene the window shows: a screen on
///   two posts showing the `Screen` texture, two of the city's cars parked in front of it
///   (demo 14's vehicle models and texture, painted with their `tint`) and the lot. The screen
///   lights the lot and the cars with the movie's average colour: the lit shaders sample the
///   scene texture's smallest mip level (its mipmaps are made every frame).
///
/// The camera stands behind the cars and sways slowly from side to side, so the screen is seen
/// at an angle.
pub struct DriveInDemo {
    drive_in: Scene,
    movie: Scene,
    screen_atlas: usize,
    room_node: usize,
    eye: Point3,
}

impl DriveInDemo {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        let drive_in = Scene::load(assets, "demo_023_drive_in_scene.json")?;
        let movie = Scene::load(assets, "demo_000_viking_room_scene.json")?;
        let screen_atlas = drive_in
            .find_atlas("Screen")
            .ok_or_else(|| anyhow!("the drive-in has no atlas `Screen`"))?;
        let room_node = movie
            .find_node("Room")
            .ok_or_else(|| anyhow!("the movie has no node `Room`"))?;
        let eye = drive_in
            .active_camera()
            .ok_or_else(|| anyhow!("the drive-in has no camera"))?
            .get_eye_position();
        Ok(Self {
            drive_in,
            movie,
            screen_atlas,
            room_node,
            eye,
        })
    }
}

impl Demo for DriveInDemo {
    fn scene(&mut self) -> &mut Scene {
        &mut self.drive_in
    }

    fn texture_scene_count(&self) -> usize {
        1
    }

    fn texture_scene_at(&mut self, _index: usize) -> (&mut Scene, SceneTextureTarget) {
        let target = SceneTextureTarget {
            scene: self.drive_in.get_id(),
            atlas: self.screen_atlas,
        };
        (&mut self.movie, target)
    }

    fn update(&mut self, time: f32) -> Result<()> {
        self.movie
            .get_node_mut(self.room_node)
            .unwrap()
            .set_transform(Mat4::from_angle_z(Deg(ROOM_SPEED * time)));
        let mut eye = self.eye;
        eye.x += SWAY * (time * SWAY_SPEED).sin();
        if let Some(camera) = self.drive_in.active_camera_mut() {
            camera.set_eye_position(eye);
        }
        Ok(())
    }
}
