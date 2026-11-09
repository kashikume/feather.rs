use anyhow::{anyhow, Result};
use cgmath::Deg;

use feather::assetsprovider::AssetsProvider;
use feather::camera::Camera;
use feather::math::{Mat4, Point3, Vec3};
use feather::perspectivecamera::PerspectiveCamera;
use feather::scene::Scene;

use super::testapp::Demo;

/// Demo 4: a lit cuboid, loaded from `demo_004_lit_cuboid_scene.json`, tumbling like demo 1.
///
/// The shader lights the cuboid (ambient, diffuse and a specular highlight) from the direction
/// in the `light` uniform, which the scene file sets on the root node; every node inherits it.
/// The highlight's `shininess` and `specularStrength` are uniforms of the material.
///
/// The scene has node `Rotation`, which this demo spins, holding node `Cube`, which has the mesh
/// and the scale (set in the scene file).
pub struct LitCuboidDemo {
    scene: Scene,
    camera: PerspectiveCamera,
    rotation_node: usize,
}

impl LitCuboidDemo {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        let scene = Scene::load(assets, "demo_004_lit_cuboid_scene.json")?;
        let rotation_node = scene
            .find_node("Rotation")
            .ok_or_else(|| anyhow!("the scene has no node `Rotation`"))?;

        let mut camera = PerspectiveCamera::new();
        camera.set_fov(45.0).set_near_far(0.1, 10.0).set_view(
            Point3::new(2.0, 2.0, 2.0),
            Point3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        );

        Ok(Self {
            scene,
            camera,
            rotation_node,
        })
    }
}

impl Demo for LitCuboidDemo {
    fn scene(&mut self) -> &mut Scene {
        &mut self.scene
    }

    fn camera(&mut self) -> &mut dyn Camera {
        &mut self.camera
    }

    fn update(&mut self, time: f32) -> Result<()> {
        // Tumble: around Z at 30 degrees per second, around X at 20. This sets only the
        // rotation; the child `Cube` node keeps the scale from the scene file.
        let rotation = Mat4::from_angle_z(Deg(30.0 * time)) * Mat4::from_angle_x(Deg(20.0 * time));
        self.scene
            .get_node_mut(self.rotation_node)
            .unwrap()
            .set_transform(rotation);
        Ok(())
    }
}
