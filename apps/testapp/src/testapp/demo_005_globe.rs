use anyhow::{anyhow, Result};
use cgmath::Deg;

use feather::assetsprovider::AssetsProvider;
use feather::camera::Camera;
use feather::math::{Mat4, Point3, Vec3};
use feather::perspectivecamera::PerspectiveCamera;
use feather::scene::Scene;

use super::testapp::Demo;

/// Degrees the globe turns around its axis per second.
const SPIN_SPEED: f32 = 18.0;
/// Radians per second along the globe's path.
const ORBIT_SPEED: f32 = 0.3;
/// Degrees the moon goes around the globe per second.
const MOON_SPEED: f32 = 24.0;

/// Demo 5: a globe with its moon, loaded from `demo_005_globe_scene.json`. The globe turns
/// around its tilted axis while drifting along an oval path, the moon goes around it, and both
/// are lit by a strong light from the side.
///
/// The world map is a texture wrapped around a ball mesh (`"sphere"` in the scene file). The
/// light comes from the `light` and `lightIntensity` uniforms on the root node; the shader
/// makes a dark night side, a highlight on water only and a glowing atmosphere at the lit edge.
///
/// The scene has node `Orbit`, which this demo moves, holding:
/// - `Tilt` (the axis tilt, set in the scene file), holding `Globe`, which this demo turns
///   around its axis;
/// - `Moon plane` (the moon's orbit tilt, set in the scene file), holding `Moon orbit`, which
///   this demo turns, holding `Moon` at a fixed distance. As `Moon orbit` turns the moon with
///   it, the moon always shows the globe the same side (the middle of its texture, with the
///   dark seas). The moon's material switches off the water highlight and the atmosphere.
pub struct GlobeDemo {
    scene: Scene,
    camera: PerspectiveCamera,
    orbit_node: usize,
    globe_node: usize,
    moon_orbit_node: usize,
}

impl GlobeDemo {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        let scene = Scene::load(assets, "demo_005_globe_scene.json")?;
        let find = |name: &str| {
            scene
                .find_node(name)
                .ok_or_else(|| anyhow!("the scene has no node `{name}`"))
        };
        let orbit_node = find("Orbit")?;
        let globe_node = find("Globe")?;
        let moon_orbit_node = find("Moon orbit")?;

        // In front of the globe's path, a little above it; the light comes from the right.
        let mut camera = PerspectiveCamera::new();
        camera.set_fov(45.0).set_near_far(0.1, 30.0).set_view(
            Point3::new(0.0, -8.0, 1.4),
            Point3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        );

        Ok(Self {
            scene,
            camera,
            orbit_node,
            globe_node,
            moon_orbit_node,
        })
    }
}

impl Demo for GlobeDemo {
    fn scene(&mut self) -> &mut Scene {
        &mut self.scene
    }

    fn camera(&mut self) -> &mut dyn Camera {
        &mut self.camera
    }

    fn update(&mut self, time: f32) -> Result<()> {
        // An oval path: left and right across the view, nearer and farther, slightly up and
        // down. The light keeps its direction, so the lit part of the globe changes with the
        // angle it is seen from.
        let angle = time * ORBIT_SPEED;
        let position = Vec3::new(
            1.3 * angle.sin(),
            // Starts at the point nearest the camera.
            -1.8 * angle.cos(),
            0.3 * (angle * 2.0).sin(),
        );
        self.scene
            .get_node_mut(self.orbit_node)
            .unwrap()
            .set_transform(Mat4::from_translation(position));
        self.scene
            .get_node_mut(self.globe_node)
            .unwrap()
            .set_transform(Mat4::from_angle_z(Deg(SPIN_SPEED * time)));
        self.scene
            .get_node_mut(self.moon_orbit_node)
            .unwrap()
            .set_transform(Mat4::from_angle_z(Deg(MOON_SPEED * time)));
        Ok(())
    }
}
