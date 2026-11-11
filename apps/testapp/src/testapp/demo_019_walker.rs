use std::f32::consts::TAU;

use anyhow::{anyhow, Result};

use feather::animationclip::AnimationClip;
use feather::assetsprovider::AssetsProvider;
use feather::math::{Mat4, Vec3};
use feather::scene::Scene;
use feather::skeleton::Skeleton;

use super::testapp::Demo;

/// Radius of the circle the person walks along.
const PATH_RADIUS: f32 = 1.7;
/// Walking speed in units per second, matched to the stride of the walk cycle so the feet
/// hardly slide.
const WALK_SPEED: f32 = 1.15;

/// Demo 19: demo 8 again, with real shadows. A textured person walking in a circle.
///
/// `demo_019_walker_scene.json` builds the same person as demo 8: body parts hanging from
/// bones, sharing the `Walker` atlas. There is no shadow disc. Every body part casts a shadow
/// (`casts_shadow`); the floor only receives. The shadow light matches demo 8's light, so the
/// walk cycle shows up on the floor.
///
/// This demo makes a `Skeleton` of the bones and plays `demo_008_walker_walk.json` on it, while
/// it moves the `Walker` node along the circle, facing where it goes.
pub struct WalkerShadowDemo {
    scene: Scene,
    skeleton: Skeleton,
    walk: AnimationClip,
    walker_node: usize,
}

impl WalkerShadowDemo {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        let scene = Scene::load(assets, "demo_019_walker_scene.json")?;
        let walker_node = scene
            .find_node("Walker")
            .ok_or_else(|| anyhow!("the scene has no node `Walker`"))?;
        let skeleton = Skeleton::from_nodes(&scene, walker_node)?;
        let walk = AnimationClip::load(assets, "demo_008_walker_walk.json")?;
        skeleton.check(&walk)?;

        let mut demo = Self {
            scene,
            skeleton,
            walk,
            walker_node,
        };
        demo.update(0.0)?;
        Ok(demo)
    }
}

impl Demo for WalkerShadowDemo {
    fn scene(&mut self) -> &mut Scene {
        &mut self.scene
    }

    fn update(&mut self, time: f32) -> Result<()> {
        // The legs and arms.
        let pose = self.walk.sample(time);
        self.skeleton.apply(&mut self.scene, &pose)?;

        // Along the circle, counter-clockwise seen from above, starting nearest the camera.
        // At `angle` it moves along (cos, sin); the person faces +Y, so it turns by
        // `angle` - 90 degrees to face where it goes.
        let angle = (WALK_SPEED * time / PATH_RADIUS) % TAU;
        let position = Vec3::new(PATH_RADIUS * angle.sin(), -PATH_RADIUS * angle.cos(), 0.0);
        let facing = Mat4::from_angle_z(cgmath::Rad(angle - TAU / 4.0));
        self.scene
            .get_node_mut(self.walker_node)
            .unwrap()
            .set_transform(Mat4::from_translation(position) * facing);
        Ok(())
    }
}
