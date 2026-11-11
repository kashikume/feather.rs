use std::f32::consts::TAU;

use anyhow::{anyhow, Result};
use cgmath::{Deg, InnerSpace};

use feather::assetsprovider::AssetsProvider;
use feather::math::{Mat4, Point3, Vec3};
use feather::scene::Scene;
use feather::shadow::ShadowLight;

use super::testapp::Demo;

/// Number of pillars, in a ring around the centre.
const PILLARS: usize = 8;
const PILLAR_RING_RADIUS: f32 = 4.2;
/// Seconds for the sun to go once round the sky.
const SUN_PERIOD: f32 = 24.0;
/// The sun's height above the horizon goes between these, in degrees.
const SUN_LOW: f32 = 22.0;
const SUN_HIGH: f32 = 58.0;
/// The part of the scene the sun's shadows cover: a sphere around the middle.
const SHADOW_CENTER: Point3 = Point3::new(0.0, 0.0, 1.0);
const SHADOW_RADIUS: f32 = 10.5;

/// Demo 14: shadows. A sun goes round the sky over a floor with a ring of stone pillars of
/// different heights, a golden cube spinning above a pedestal and three bouncing balls; their
/// shadows turn and stretch with it.
///
/// The scene file has the meshes, the shader, the materials, the floor, the pedestal and the
/// cube, and a `shadow_light`; this demo adds the pillars and balls, all casting shadows
/// (`Node::set_casts_shadow`), and moves the sun every frame (`Scene::set_shadow_light`, and
/// the `light` uniform on the root node for the lighting). The floor receives shadows but
/// casts none.
///
/// The engine draws the casters from the sun into a shadow map before each frame; the shader
/// (`demo_014_shadows_shader.frag`) compares each fragment's depth seen from the sun with the
/// map, averaging 3x3 samples to soften the edges.
pub struct ShadowsDemo {
    scene: Scene,
    root: usize,
    cube: usize,
    balls: Vec<(usize, f32)>,
}

impl ShadowsDemo {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        let mut scene = Scene::load(assets, "demo_014_shadows_scene.json")?;
        let find = |what: &str, handle: Option<usize>| {
            handle.ok_or_else(|| anyhow!("the scene has no {what}"))
        };
        let root = find("node `Scene root`", scene.find_node("Scene root"))?;
        let pillars = find("node `Pillars`", scene.find_node("Pillars"))?;
        let balls_node = find("node `Balls`", scene.find_node("Balls"))?;
        let cube = find("node `Cube`", scene.find_node("Cube"))?;
        let pillar_mesh = find("mesh `Pillar`", scene.find_mesh("Pillar"))?;
        let ball_mesh = find("mesh `Ball`", scene.find_mesh("Ball"))?;
        let matte = find("material `Matte`", scene.find_material("Matte"))?;
        let glossy = find("material `Glossy`", scene.find_material("Glossy"))?;

        // A ring of pillars, of heights going up and down around it.
        for i in 0..PILLARS {
            let angle = i as f32 / PILLARS as f32 * TAU;
            let height = 1.6 + 1.4 * (0.5 + 0.5 * (angle * 2.0).sin());
            let node = scene.create_node(Some(format!("Pillar {}", i + 1)), pillars)?;
            scene.node_set_mesh(node, pillar_mesh)?;
            scene.node_set_material(node, matte)?;
            let node = scene.get_node_mut(node).unwrap();
            node.set_casts_shadow(true);
            node.set_transform(
                Mat4::from_translation(Vec3::new(
                    PILLAR_RING_RADIUS * angle.cos(),
                    PILLAR_RING_RADIUS * angle.sin(),
                    0.0,
                )) * Mat4::from_nonuniform_scale(1.0, 1.0, height),
            );
        }

        // Balls bouncing between the pedestal and the pillars, each at its own pace.
        let mut balls = Vec::new();
        let colours = [
            Vec3::new(0.85, 0.2, 0.18),
            Vec3::new(0.2, 0.45, 0.9),
            Vec3::new(0.25, 0.75, 0.35),
        ];
        for (i, colour) in colours.into_iter().enumerate() {
            let node = scene.create_node(Some(format!("Ball {}", i + 1)), balls_node)?;
            scene.node_set_mesh(node, ball_mesh)?;
            scene.node_set_material(node, glossy)?;
            let n = scene.get_node_mut(node).unwrap();
            n.set_casts_shadow(true);
            n.uniforms_mut().set("color", colour);
            balls.push((node, i as f32 * 0.37));
        }

        let mut demo = Self {
            scene,
            root,
            cube,
            balls,
        };
        demo.update(0.0)?;
        Ok(demo)
    }

    /// The direction the sun's rays go at `time` (towards the ground).
    fn sun_direction(time: f32) -> Vec3 {
        let azimuth = time / SUN_PERIOD * TAU;
        let wave = 0.5 + 0.5 * (time / SUN_PERIOD * TAU * 2.0).sin();
        let elevation = (SUN_LOW + (SUN_HIGH - SUN_LOW) * wave).to_radians();
        // Towards the sun, then reversed.
        let to_sun = Vec3::new(
            elevation.cos() * azimuth.cos(),
            elevation.cos() * azimuth.sin(),
            elevation.sin(),
        );
        -to_sun.normalize()
    }
}

impl Demo for ShadowsDemo {
    fn scene(&mut self) -> &mut Scene {
        &mut self.scene
    }

    fn update(&mut self, time: f32) -> Result<()> {
        let sun = Self::sun_direction(time);
        let map_size = self.scene.get_shadow_light().map(|l| l.map_size);
        let mut light = ShadowLight::directional(sun, SHADOW_CENTER, SHADOW_RADIUS);
        if let Some(size) = map_size {
            light = light.with_map_size(size);
        }
        self.scene.set_shadow_light(Some(light));
        self.scene
            .get_node_mut(self.root)
            .unwrap()
            .uniforms_mut()
            .set("light", -sun);

        // The cube spins and hovers above the pedestal.
        self.scene.get_node_mut(self.cube).unwrap().set_transform(
            Mat4::from_translation(Vec3::new(0.0, 0.0, 1.55 + 0.25 * (time * 1.3).sin()))
                * Mat4::from_angle_z(Deg(40.0 * time))
                * Mat4::from_angle_x(Deg(25.0 * time)),
        );

        // The balls bounce along their own lines.
        for (i, &(ball, phase)) in self.balls.iter().enumerate() {
            let t = time * 0.9 + phase;
            let bounce = ((t * 3.1).sin()).abs() * 2.2;
            let angle = i as f32 / self.balls.len() as f32 * TAU + 0.6;
            let along = 2.3 + 0.6 * (t * 0.7).sin();
            self.scene
                .get_node_mut(ball)
                .unwrap()
                .set_transform(Mat4::from_translation(Vec3::new(
                    along * angle.cos(),
                    along * angle.sin(),
                    0.5 + bounce,
                )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sun_stays_above_the_horizon_and_goes_round() {
        let mut azimuths = Vec::new();
        for step in 0..48 {
            let time = step as f32 * SUN_PERIOD / 48.0;
            let rays = ShadowsDemo::sun_direction(time);
            assert!((rays.magnitude() - 1.0).abs() < 1e-5);
            let elevation = (-rays.z).asin().to_degrees();
            assert!(
                (SUN_LOW - 0.01..=SUN_HIGH + 0.01).contains(&elevation),
                "{elevation}"
            );
            azimuths.push((-rays.y).atan2(-rays.x));
        }
        // Every quarter of the sky is visited.
        for quarter in 0..4 {
            let low = -std::f32::consts::PI + quarter as f32 * std::f32::consts::FRAC_PI_2;
            let high = low + std::f32::consts::FRAC_PI_2;
            assert!(
                azimuths.iter().any(|a| (low..high).contains(a)),
                "quarter {quarter}"
            );
        }
    }
}
