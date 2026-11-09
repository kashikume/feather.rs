use std::f32::consts::TAU;

use anyhow::{anyhow, Result};

use feather::animationclip::AnimationClip;
use feather::assetsprovider::AssetsProvider;
use feather::camera::Camera;
use feather::math::{Mat4, Point3, Vec3};
use feather::perspectivecamera::PerspectiveCamera;
use feather::scene::Scene;
use feather::skeleton::Skeleton;

use super::testapp::Demo;

/// Radius of the circle the person walks along.
const PATH_RADIUS: f32 = 1.7;
/// Walking speed in units per second, matched to the stride of the walk cycle so the feet
/// hardly slide.
const WALK_SPEED: f32 = 1.15;

/// Demo 8: a textured person walking in a circle, animated with a simple bone system.
///
/// `demo_008_walker_scene.json` builds the person from body parts (generated OBJ meshes and
/// spheres) hanging from bones: the named nodes under `Walker` (`Hips`, `Spine`, `LeftHip`,
/// `LeftKnee`, ...), placed at the joints. All parts share one atlas, built during the build
/// from images in `atlases/demo_008_walker/`; each material shows one texture of it.
///
/// This demo makes a `Skeleton` of the bones and plays the walk cycle from
/// `demo_008_walker_walk.json` on it (rotations of the joints and a little up and down of the
/// hips), while it moves the `Walker` node along the circle, facing where it goes.
pub struct WalkerDemo {
    scene: Scene,
    camera: PerspectiveCamera,
    skeleton: Skeleton,
    walk: AnimationClip,
    walker_node: usize,
}

impl WalkerDemo {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        let scene = Scene::load(assets, "demo_008_walker_scene.json")?;
        let walker_node = scene
            .find_node("Walker")
            .ok_or_else(|| anyhow!("the scene has no node `Walker`"))?;
        let skeleton = Skeleton::from_nodes(&scene, walker_node)?;
        let walk = AnimationClip::load(assets, "demo_008_walker_walk.json")?;
        skeleton.check(&walk)?;

        let mut camera = PerspectiveCamera::new();
        camera.set_fov(45.0).set_near_far(0.1, 50.0).set_view(
            Point3::new(0.0, -5.6, 2.4),
            Point3::new(0.0, 0.0, 0.8),
            Vec3::new(0.0, 0.0, 1.0),
        );

        let mut demo = Self {
            scene,
            camera,
            skeleton,
            walk,
            walker_node,
        };
        demo.update(0.0)?;
        Ok(demo)
    }
}

impl Demo for WalkerDemo {
    fn scene(&mut self) -> &mut Scene {
        &mut self.scene
    }

    fn camera(&mut self) -> &mut dyn Camera {
        &mut self.camera
    }

    fn update(&mut self, time: f32) -> Result<()> {
        // The legs and arms.
        let pose = self.walk.sample(time);
        self.skeleton.apply(&mut self.scene, &pose)?;

        // Along the circle, counter-clockwise seen from above, starting nearest the camera.
        // At `angle` it moves along (cos, sin); the person faces +Y, so it turns by
        // `angle` - 90 degrees to face where it goes.
        let angle = (WALK_SPEED * time / PATH_RADIUS) % TAU;
        let position = Vec3::new(
            PATH_RADIUS * angle.sin(),
            -PATH_RADIUS * angle.cos(),
            0.0,
        );
        let facing = Mat4::from_angle_z(cgmath::Rad(angle - TAU / 4.0));
        self.scene
            .get_node_mut(self.walker_node)
            .unwrap()
            .set_transform(Mat4::from_translation(position) * facing);
        Ok(())
    }
}
