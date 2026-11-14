use std::f32::consts::TAU;

use anyhow::{anyhow, bail, Result};

use feather::assetsprovider::AssetsProvider;
use feather::math::{Mat4, Vec3};
use feather::scene::Scene;

use super::random::Random;
use super::testapp::Demo;

/// Number of plates in a row and in a column.
const GRID: usize = 4;
/// Distance between the centres of neighbouring plates (plates are 1x1).
const SPACING: f32 = 1.3;

/// Demo 3: a waving atlas. 16 plates hang in a 4x4 grid, each showing one texture of the atlas, and
/// wave in the wind.
///
/// The atlas is built during the build from the images listed in
/// `atlases/demo_003_waving_atlas_atlas.atlist`. The scene file has the atlas, the plate mesh, the
/// waving shader and the `Plates` node; this demo creates a material and a node per plate,
/// with the texture from `Atlas::get_texture` and random wind settings (`phase`, `speed`,
/// `strength`). The shader moves the plates; this demo only passes it the `time`.
pub struct WavingAtlasDemo {
    scene: Scene,
    plates_node: usize,
}

impl WavingAtlasDemo {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        let mut scene = Scene::load(assets, "demo_003_waving_atlas_scene.json")?;
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
            // The shader bends the flat plate out of its mesh's (flat) bounds: never cull it.
            node.set_culled(false);
            node.uniforms_mut()
                .set("phase", random.next() * TAU)
                .set("speed", 1.0 + random.next() * 1.2)
                .set("strength", 0.3 + random.next() * 0.3);
        }

        // The scene file's camera: in front of the grid and a little to the side, so that the
        // wind's push is visible, looking at its centre. The `Plates` node turns the plates from
        // the XY plane to the XZ plane, facing -Y.
        Ok(Self { scene, plates_node })
    }
}

impl Demo for WavingAtlasDemo {
    fn scene(&mut self) -> &mut Scene {
        &mut self.scene
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
