use std::f32::consts::TAU;

use anyhow::{anyhow, Result};

use feather::animationclip::AnimationClip;
use feather::assetsprovider::AssetsProvider;
use feather::math::{Mat4, Vec3};
use feather::scene::Scene;
use feather::skeleton::Skeleton;

use super::testapp::Demo;

/// Radius of the circle the spider walks along.
const PATH_RADIUS: f32 = 1.9;
/// Walking speed in units per second, matched to how far the feet push back in a step, so they
/// hardly slide.
const WALK_SPEED: f32 = 0.6;

/// Demo 9: a textured spider walking in a circle, with the same bone system as the walker.
///
/// `demo_009_spider_scene.json` builds the spider: a body of scaled spheres (head and thorax
/// with eight eyes, and the abdomen) and eight legs of three generated segments each, hanging
/// from bones. Each leg has four bones: `<leg>Hip` turns it forward and back around the
/// vertical axis, `<leg>Femur` raises it, `<leg>Knee` and `<leg>Ankle` bend it down to the floor
/// (legs are `L1`..`L4` on the left and `R1`..`R4` on the right, front to back). All parts share
/// one atlas, built during the build from images in `atlases/demo_009_spider/`.
///
/// The walk cycle (`demo_009_spider_walk.json`) is the alternating tetrapod gait of real
/// spiders: L1, R2, L3 and R4 step together, then the other four, so four feet are always on
/// the floor. This demo plays it on the spider's skeleton and moves the `Spider` node along the
/// circle, facing where it goes.
pub struct SpiderDemo {
    scene: Scene,
    skeleton: Skeleton,
    walk: AnimationClip,
    spider_node: usize,
}

impl SpiderDemo {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        let scene = Scene::load(assets, "demo_009_spider_scene.json")?;
        let spider_node = scene
            .find_node("Spider")
            .ok_or_else(|| anyhow!("the scene has no node `Spider`"))?;
        let skeleton = Skeleton::from_nodes(&scene, spider_node)?;
        let walk = AnimationClip::load(assets, "demo_009_spider_walk.json")?;
        skeleton.check(&walk)?;

        let mut demo = Self {
            scene,
            skeleton,
            walk,
            spider_node,
        };
        demo.update(0.0)?;
        Ok(demo)
    }
}

impl Demo for SpiderDemo {
    fn scene(&mut self) -> &mut Scene {
        &mut self.scene
    }

    fn update(&mut self, time: f32) -> Result<()> {
        // The legs, body and abdomen.
        let pose = self.walk.sample(time);
        self.skeleton.apply(&mut self.scene, &pose)?;

        // Along the circle, counter-clockwise seen from above, starting nearest the camera.
        // At `angle` it moves along (cos, sin); the spider faces +Y, so it turns by
        // `angle` - 90 degrees to face where it goes.
        let angle = (WALK_SPEED * time / PATH_RADIUS) % TAU;
        let position = Vec3::new(
            PATH_RADIUS * angle.sin(),
            -PATH_RADIUS * angle.cos(),
            0.0,
        );
        let facing = Mat4::from_angle_z(cgmath::Rad(angle - TAU / 4.0));
        self.scene
            .get_node_mut(self.spider_node)
            .unwrap()
            .set_transform(Mat4::from_translation(position) * facing);
        Ok(())
    }
}
