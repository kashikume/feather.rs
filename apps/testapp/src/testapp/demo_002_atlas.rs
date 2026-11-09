use std::f32::consts::TAU;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Result};

use feather::assetsprovider::AssetsProvider;
use feather::camera::Camera;
use feather::math::{Mat4, Point3, Vec3};
use feather::perspectivecamera::PerspectiveCamera;
use feather::scene::Scene;

use super::testapp::Demo;

/// Number of plates in a row and in a column.
const GRID: usize = 4;
/// Distance between the centres of neighbouring plates (plates are 1x1).
const SPACING: f32 = 1.3;

/// Demo 2: an atlas. 16 plates hang in a 4x4 grid, each showing one texture of the atlas, and
/// wave in the wind.
///
/// The atlas is built during the build from the images listed in
/// `atlases/demo_002_atlas_atlas.atlist`. The scene file has the atlas, the plate mesh, the
/// waving shader and the `Plates` node; this demo creates a material and a node per plate,
/// with the texture from `Atlas::get_texture` and random wind settings (`phase`, `speed`,
/// `strength`). The shader moves the plates; this demo only passes it the `time`.
pub struct AtlasDemo {
    scene: Scene,
    camera: PerspectiveCamera,
    plates_node: usize,
}

impl AtlasDemo {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        let mut scene = Scene::load(assets, "demo_002_atlas_scene.json")?;
        let find = |what: &str, handle: Option<usize>| {
            handle.ok_or_else(|| anyhow!("the scene has no {what}"))
        };
        let plates_node = find("node `Plates`", scene.find_node("Plates"))?;
        let atlas = find("atlas `Plates`", scene.find_atlas("Plates"))?;
        let shader = find("shader `Waving`", scene.find_shader("Waving"))?;
        let mesh = find("mesh `Plate`", scene.find_mesh("Plate"))?;

        let textures: Vec<_> = {
            let atlas = scene.get_atlas(atlas).unwrap();
            atlas
                .get_texture_names()
                .into_iter()
                .map(|name| (name.to_string(), atlas.get_texture(name).unwrap()))
                .collect()
        };
        if textures.len() < GRID * GRID {
            bail!(
                "atlas `Plates` has {} textures, the demo needs {}",
                textures.len(),
                GRID * GRID
            );
        }

        let mut random = Random::from_clock();
        for (i, (name, texture)) in textures.into_iter().take(GRID * GRID).enumerate() {
            let material = scene.create_material(Some(name.clone()), shader)?;
            scene.material_set_texture(material, Some(texture))?;

            let node = scene.create_node(Some(name), plates_node)?;
            scene.node_set_mesh(node, mesh)?;
            scene.node_set_material(node, material)?;

            // Row 0 at the top. The mesh hangs down from y = 0, so the node is at its top edge.
            let (column, row) = ((i % GRID) as f32, (i / GRID) as f32);
            let middle = (GRID - 1) as f32 / 2.0;
            let top = Vec3::new(
                (column - middle) * SPACING,
                (middle - row) * SPACING + 0.5,
                0.0,
            );
            let node = scene.get_node_mut(node).unwrap();
            node.set_transform(Mat4::from_translation(top));
            node.uniforms_mut()
                .set("phase", random.next() * TAU)
                .set("speed", 1.0 + random.next() * 1.2)
                .set("strength", 0.3 + random.next() * 0.3);
        }

        // In front of the grid and a little to the side, so that the wind's push is visible,
        // looking at its centre. The `Plates` node turns the plates from the XY plane to the XZ
        // plane, facing -Y.
        let mut camera = PerspectiveCamera::new();
        camera.set_fov(45.0).set_near_far(0.1, 20.0).set_view(
            Point3::new(3.2, -6.4, 1.6),
            Point3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        );

        Ok(Self {
            scene,
            camera,
            plates_node,
        })
    }
}

impl Demo for AtlasDemo {
    fn scene(&mut self) -> &mut Scene {
        &mut self.scene
    }

    fn camera(&mut self) -> &mut dyn Camera {
        &mut self.camera
    }

    fn update(&mut self, time: f32) -> Result<()> {
        // Every plate inherits `time` from the `Plates` node.
        self.scene
            .get_node_mut(self.plates_node)
            .unwrap()
            .uniforms_mut()
            .set("time", time);
        Ok(())
    }
}

/// A small pseudo-random number generator (xorshift32), enough for wind settings.
struct Random(u32);

impl Random {
    /// Seeded from the clock, so the plates wave differently every run.
    fn from_clock() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        Self(nanos | 1)
    }

    /// A number in `0.0..1.0`.
    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::Random;

    #[test]
    fn random_numbers_are_in_range_and_vary() {
        let mut random = Random(12345);
        let values: Vec<f32> = (0..1000).map(|_| random.next()).collect();
        assert!(values.iter().all(|v| (0.0..1.0).contains(v)));
        let mean = values.iter().sum::<f32>() / values.len() as f32;
        assert!((0.4..0.6).contains(&mean), "{mean}");
    }
}
