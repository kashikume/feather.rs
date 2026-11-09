use anyhow::{anyhow, Result};
use winit::keyboard::KeyCode;

use feather::assetsprovider::{create_assets_provider, AssetsProvider};
use feather::camera::Camera;
use feather::featherapp::FeatherApp;
use feather::scene::Scene;

use super::demo_000_viking_room::VikingRoomDemo;
use super::demo_001_cuboid::CuboidDemo;

/// One demo of the test app: a scene, the camera looking at it, and its animation.
pub trait Demo {
    fn scene(&mut self) -> &mut Scene;
    fn camera(&mut self) -> &mut dyn Camera;
    /// Called every frame; `time` is in seconds since the demo started.
    fn update(&mut self, time: f32) -> Result<()>;
}

/// An entry of the demo list.
pub struct DemoInfo {
    pub name: &'static str,
    pub create: fn(&dyn AssetsProvider) -> Result<Box<dyn Demo>>,
}

/// All demos, by number.
pub const DEMOS: &[DemoInfo] = &[
    DemoInfo {
        name: "Viking room",
        create: |assets| Ok(Box::new(VikingRoomDemo::new(assets)?)),
    },
    DemoInfo {
        name: "Cuboid",
        create: |assets| Ok(Box::new(CuboidDemo::new(assets)?)),
    },
];

/// The development app: shows one demo at a time. Left and right arrows switch to the
/// previous and next demo, wrapping around.
pub struct TestApp {
    assets: Box<dyn AssetsProvider>,
    demo_index: usize,
    demo: Box<dyn Demo>,
    /// App time (as passed to `on_update`) when the current demo started; `None` until its
    /// first update.
    demo_start: Option<f32>,
}

impl FeatherApp for TestApp {
    fn on_create(&mut self) -> Result<()> {
        log::trace!("on_create called");
        Ok(())
    }

    fn on_render(&mut self) -> Result<()> {
        log::trace!("on_render called");
        Ok(())
    }

    fn on_update(&mut self, time: f32) -> Result<()> {
        log::trace!("on_update called with time: {}", time);
        let start = *self.demo_start.get_or_insert(time);
        self.demo.update(time - start)
    }

    fn on_destroy(&mut self) {
        log::trace!("on_destroy called");
    }

    fn get_num_scenes_to_render(&self) -> usize {
        1
    }

    fn get_scene_to_render(&mut self, _scene_index: usize) -> &mut Scene {
        self.demo.scene()
    }

    fn get_camera_to_render_scene(&mut self, _scene_index: usize) -> &mut dyn Camera {
        self.demo.camera()
    }

    fn on_key_pressed(&mut self, key: KeyCode) -> Result<()> {
        let count = DEMOS.len();
        match key {
            KeyCode::ArrowRight => self.show_demo((self.demo_index + 1) % count),
            KeyCode::ArrowLeft => self.show_demo((self.demo_index + count - 1) % count),
            _ => Ok(()),
        }
    }
}

impl TestApp {
    /// Creates the app showing demo `demo_index` (see `DEMOS`).
    pub fn new(demo_index: usize) -> Result<Self> {
        let assets = create_assets_provider("assets")?;
        let demo = create_demo(&*assets, demo_index)?;
        Ok(Self {
            assets,
            demo_index,
            demo,
            demo_start: None,
        })
    }

    /// Replaces the current demo with demo `index`. If it can't be created, the current demo
    /// stays and the error is returned.
    fn show_demo(&mut self, index: usize) -> Result<()> {
        self.demo = create_demo(&*self.assets, index)?;
        self.demo_index = index;
        self.demo_start = None;
        Ok(())
    }
}

fn create_demo(assets: &dyn AssetsProvider, index: usize) -> Result<Box<dyn Demo>> {
    let info = DEMOS
        .get(index)
        .ok_or_else(|| anyhow!("there is no demo {index}"))?;
    log::info!("Demo {index}: {}", info.name);
    (info.create)(assets)
        .map_err(|e| e.context(format!("Cannot create demo {index} ({})", info.name)))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use anyhow::Context;
    use cgmath::SquareMatrix;
    use feather::math::Mat4;
    use feather::object::Object;

    use super::*;

    /// The assets the app gets at runtime: `resources/` plus the shaders compiled by `build.rs`.
    struct SourceAssets;

    impl AssetsProvider for SourceAssets {
        fn get_asset(&self, name: &str) -> Result<Vec<u8>> {
            let dir = if name.ends_with(".spv") {
                concat!(env!("OUT_DIR"), "/shaders")
            } else {
                concat!(env!("CARGO_MANIFEST_DIR"), "/resources")
            };
            std::fs::read(Path::new(dir).join(name)).with_context(|| format!("asset {name}"))
        }
    }

    fn demo(index: usize) -> Box<dyn Demo> {
        (DEMOS[index].create)(&SourceAssets).unwrap()
    }

    #[test]
    fn every_demo_loads() {
        for index in 0..DEMOS.len() {
            demo(index);
        }
    }

    #[test]
    fn viking_room_scene_loads() {
        let mut demo = demo(0);
        let scene = demo.scene();

        let room = scene.get_node(scene.find_node("Room").unwrap()).unwrap();
        assert!(room.get_mesh().is_some());
        assert!(room.get_material().is_some());
        let parent = scene.get_node(room.get_parent().unwrap()).unwrap();
        assert_eq!(parent.get_name(), Some("Scene root".to_string()));
    }

    #[test]
    fn cuboid_demo_scene_loads() {
        let mut demo = demo(1);
        let scene = demo.scene();

        // `Rotation` (spun by the demo) holds `Cube`, which has the mesh and the scale.
        let rotation = scene.find_node("Rotation").unwrap();
        let cube = scene.get_node(scene.find_node("Cube").unwrap()).unwrap();
        assert_eq!(cube.get_parent(), Some(rotation));
        assert!(cube.get_mesh().is_some());
        assert!(cube.get_material().is_some());
        assert_ne!(cube.get_transform(), Mat4::identity());
    }
}
