use anyhow::Result;

use feather::assetsprovider::AssetsProvider;
use feather::scene::Scene;

use super::testapp::Demo;

/// Demo 13: the simplest shadow. A human figure standing on a floor, lit from the side, casting
/// its shadow on the floor. Nothing moves.
///
/// Everything is in `demo_013_simple_shadow_scene.json`:
/// - `shadow_light`: a directional light; the engine draws the shadow casters from it into a
///   shadow map before each frame;
/// - the `Human` node has `"casts_shadow": true`; the floor doesn't cast, it only receives;
/// - the shader names the uniform and sampler the engine fills: the light's matrix
///   (`shadow_matrix_uniform`) and the shadow map (`shadow_map_sampler`).
///
/// The shader (`demo_013_simple_shadow_shader.frag`) looks up each fragment in the shadow map
/// once and keeps the light off where something nearer to the light was drawn. The figure
/// (`demo_013_simple_shadow_human_mesh.obj`) is one generated mesh of overlapping ellipsoids.
pub struct SimpleShadowDemo {
    scene: Scene,
}

impl SimpleShadowDemo {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        let scene = Scene::load(assets, "demo_013_simple_shadow_scene.json")?;

        Ok(Self { scene })
    }
}

impl Demo for SimpleShadowDemo {
    fn scene(&mut self) -> &mut Scene {
        &mut self.scene
    }

    fn update(&mut self, _time: f32) -> Result<()> {
        Ok(())
    }
}
