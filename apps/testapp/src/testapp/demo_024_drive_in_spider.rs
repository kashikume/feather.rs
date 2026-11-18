use anyhow::{anyhow, Result};

use feather::assetsprovider::AssetsProvider;
use feather::featherapp::SceneTextureTarget;
use feather::math::Point3;
use feather::scene::Scene;

use super::demo_019_spider::SpiderShadowDemo;
use super::testapp::Demo;

/// How far the camera sways to each side (scene units), and how fast (radians per second).
const SWAY: f32 = 3.5;
const SWAY_SPEED: f32 = 0.15;

/// Demo 24: the drive-in again (demo 23), showing demo 19's spider walking with its shadow, on
/// a canvas screen waving in the wind.
///
/// Two scenes:
/// - the **movie** is demo 19 itself (`SpiderShadowDemo`), updated by this demo: its spider
///   walks its circle, casting its shadow into the movie's own shadow map, drawn before the
///   movie is drawn into the drive-in's scene texture `Screen` every frame;
/// - the **drive-in** (`demo_024_drive_in_spider_scene.json`) is demo 23's lot, cars and
///   lighting, but the screen is a canvas hanging from a beam between two posts, a grid that
///   the vertex shader bends in the wind as demo 3 bends its plates
///   (`demo_024_drive_in_spider_screen_shader.vert`). It gets the time from this demo
///   (`time` on its node); its folds are lit brighter or darker as they face the projector,
///   which stands behind the cars.
pub struct DriveInSpiderDemo {
    drive_in: Scene,
    movie: SpiderShadowDemo,
    screen_atlas: usize,
    canvas_node: usize,
    eye: Point3,
}

impl DriveInSpiderDemo {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        let drive_in = Scene::load(assets, "demo_024_drive_in_spider_scene.json")?;
        let movie = SpiderShadowDemo::new(assets)?;
        let screen_atlas = drive_in
            .find_atlas("Screen")
            .ok_or_else(|| anyhow!("the drive-in has no atlas `Screen`"))?;
        let canvas_node = drive_in
            .find_node("Canvas")
            .ok_or_else(|| anyhow!("the drive-in has no node `Canvas`"))?;
        let eye = drive_in
            .active_camera()
            .ok_or_else(|| anyhow!("the drive-in has no camera"))?
            .get_eye_position();
        Ok(Self {
            drive_in,
            movie,
            screen_atlas,
            canvas_node,
            eye,
        })
    }
}

impl Demo for DriveInSpiderDemo {
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
        (self.movie.scene(), target)
    }

    fn update(&mut self, time: f32) -> Result<()> {
        self.movie.update(time)?;
        self.drive_in
            .get_node_mut(self.canvas_node)
            .unwrap()
            .uniforms_mut()
            .set("time", time);
        let mut eye = self.eye;
        eye.x += SWAY * (time * SWAY_SPEED).sin();
        if let Some(camera) = self.drive_in.active_camera_mut() {
            camera.set_eye_position(eye);
        }
        Ok(())
    }
}
