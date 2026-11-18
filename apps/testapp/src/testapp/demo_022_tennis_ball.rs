use anyhow::{anyhow, Result};
use cgmath::Deg;

use feather::assetsprovider::AssetsProvider;
use feather::math::Mat4;
use feather::scene::Scene;

use super::testapp::Demo;

/// Degrees the ball turns around its axis per second.
const SPIN_SPEED: f32 = 22.0;
/// Degrees the axis sways to each side, and how fast (radians per second).
const SWAY: f32 = 12.0;
const SWAY_SPEED: f32 = 0.35;

/// Demo 22: a tennis ball turning under a light from the side, loaded from
/// `demo_022_tennis_ball_scene.json`; a material with two textures.
///
/// The ball's material fills two texture slots: slot 0 is the felt and seam colours
/// (`tennis_ball.png`), slot 1 a tangent-space normal map (`tennis_ball_normal.png`, an atlas
/// with `"color_space": "linear"`, so the shader reads its values as they are stored). The
/// shader lists its samplers in that order (`texture_samplers`). It bends the ball's normals
/// with the map, building the tangent frame from screen-space derivatives (the sphere mesh has
/// no tangents), so the seam's groove and the felt's fuzz catch and lose the light as the ball
/// turns. Both images are made by `tools/scripts/make_tennis_ball_textures.py`.
///
/// The scene has node `Tilt` (the axis tilt, set in the scene file), holding `Ball`, which this
/// demo turns around its axis while the axis sways slowly.
pub struct TennisBallDemo {
    scene: Scene,
    tilt_node: usize,
    ball_node: usize,
    tilt: Mat4,
}

impl TennisBallDemo {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        let scene = Scene::load(assets, "demo_022_tennis_ball_scene.json")?;
        let find = |name: &str| {
            scene
                .find_node(name)
                .ok_or_else(|| anyhow!("the scene has no node `{name}`"))
        };
        let tilt_node = find("Tilt")?;
        let ball_node = find("Ball")?;
        let tilt = scene.get_node(tilt_node).unwrap().get_transform();
        Ok(Self {
            scene,
            tilt_node,
            ball_node,
            tilt,
        })
    }
}

impl Demo for TennisBallDemo {
    fn scene(&mut self) -> &mut Scene {
        &mut self.scene
    }

    fn update(&mut self, time: f32) -> Result<()> {
        let sway = Mat4::from_angle_y(Deg(SWAY * (time * SWAY_SPEED).sin()));
        self.scene
            .get_node_mut(self.tilt_node)
            .unwrap()
            .set_transform(sway * self.tilt);
        self.scene
            .get_node_mut(self.ball_node)
            .unwrap()
            .set_transform(Mat4::from_angle_z(Deg(SPIN_SPEED * time)));
        Ok(())
    }
}
