use anyhow::{anyhow, bail, Result};

use feather::assetsprovider::AssetsProvider;
use feather::math::{Mat4, Vec3};
use feather::scene::Scene;

use super::testapp::Demo;

/// Number of plates in a row and in a column.
const GRID: usize = 4;
/// Distance between the centres of neighbouring plates (plates are 1x1).
const SPACING: f32 = 1.2;

/// Demo 2: an atlas. 16 flat plates in a 4x4 grid, each showing one texture of the atlas.
/// Nothing moves.
///
/// The atlas is built during the build from the images listed in
/// `atlases/demo_002_atlas_atlas.atlist`. The scene file has the atlas, the plate mesh, the
/// shader and the `Plates` node; this demo creates a material and a node per plate, with the
/// texture from `Atlas::get_texture`.
pub struct AtlasDemo {
    scene: Scene,
}

impl AtlasDemo {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        let mut scene = Scene::load(assets, "demo_002_atlas_scene.json")?;
        let find = |what: &str, handle: Option<usize>| {
            handle.ok_or_else(|| anyhow!("the scene has no {what}"))
        };
        let plates_node = find("node `Plates`", scene.find_node("Plates"))?;
        let atlas = find("atlas `Plates`", scene.find_atlas("Plates"))?;
        let shader = find("shader `Textured`", scene.find_shader("Textured"))?;
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

        for (i, (name, texture)) in textures.into_iter().take(GRID * GRID).enumerate() {
            let material = scene.create_material(Some(name.clone()), shader)?;
            scene.material_set_texture(material, Some(texture))?;

            let node = scene.create_node(Some(name), plates_node)?;
            scene.node_set_mesh(node, mesh)?;
            scene.node_set_material(node, material)?;

            // Row 0 at the top; the mesh is centred on the node.
            let (column, row) = ((i % GRID) as f32, (i / GRID) as f32);
            let middle = (GRID - 1) as f32 / 2.0;
            let centre = Vec3::new((column - middle) * SPACING, (middle - row) * SPACING, 0.0);
            scene
                .get_node_mut(node)
                .unwrap()
                .set_transform(Mat4::from_translation(centre));
        }

        // The scene file's camera: straight in front of the grid. The `Plates` node turns the
        // plates from the XY plane to the XZ plane, facing -Y.
        Ok(Self { scene })
    }
}

impl Demo for AtlasDemo {
    fn scene(&mut self) -> &mut Scene {
        &mut self.scene
    }

    fn update(&mut self, _time: f32) -> Result<()> {
        Ok(())
    }
}
