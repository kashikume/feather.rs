use anyhow::{anyhow, Result};
use winit::keyboard::KeyCode;

use feather::assetsprovider::{create_assets_provider, AssetsProvider};
use feather::camera::Camera;
use feather::featherapp::FeatherApp;
use feather::scene::Scene;

use super::demo_000_viking_room::VikingRoomDemo;
use super::demo_001_cuboid::CuboidDemo;
use super::demo_002_atlas::AtlasDemo;
use super::demo_003_waving_atlas::WavingAtlasDemo;
use super::demo_004_lit_cuboid::LitCuboidDemo;
use super::demo_005_globe::GlobeDemo;
use super::demo_006_bouncing_balls::BouncingBallsDemo;
use super::demo_007_vases::VasesDemo;

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
    DemoInfo {
        name: "Atlas",
        create: |assets| Ok(Box::new(AtlasDemo::new(assets)?)),
    },
    DemoInfo {
        name: "Waving atlas",
        create: |assets| Ok(Box::new(WavingAtlasDemo::new(assets)?)),
    },
    DemoInfo {
        name: "Lit cuboid",
        create: |assets| Ok(Box::new(LitCuboidDemo::new(assets)?)),
    },
    DemoInfo {
        name: "Globe",
        create: |assets| Ok(Box::new(GlobeDemo::new(assets)?)),
    },
    DemoInfo {
        name: "Bouncing balls",
        create: |assets| Ok(Box::new(BouncingBallsDemo::new(assets)?)),
    },
    DemoInfo {
        name: "Vases",
        create: |assets| Ok(Box::new(VasesDemo::new(assets)?)),
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

    /// The assets the app gets at runtime: `resources/` plus the shaders and atlases built by
    /// `build.rs`.
    struct SourceAssets;

    impl AssetsProvider for SourceAssets {
        fn get_asset(&self, name: &str) -> Result<Vec<u8>> {
            let dirs = [
                concat!(env!("CARGO_MANIFEST_DIR"), "/resources"),
                concat!(env!("OUT_DIR"), "/shaders"),
                concat!(env!("OUT_DIR"), "/atlases"),
            ];
            let path = dirs
                .iter()
                .map(|dir| Path::new(dir).join(name))
                .find(|path| path.is_file())
                .with_context(|| format!("asset {name} not found"))?;
            std::fs::read(&path).with_context(|| format!("asset {name}"))
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

    #[test]
    fn lit_cuboid_demo_inherits_the_light_from_the_root() {
        let mut demo = demo(4);
        demo.update(1.0).unwrap();
        let scene = demo.scene();
        scene.update_nodes();

        let root = scene.get_node(scene.find_node("Scene root").unwrap()).unwrap();
        let light = feather::uniform::UniformValue::Vec3(feather::math::Vec3::new(1.0, -0.5, 2.0));
        assert_eq!(root.uniforms().get("light"), Some(&light));

        let cube = scene.get_node(scene.find_node("Cube").unwrap()).unwrap();
        assert!(cube.uniforms().get("light").is_none());
        assert_eq!(cube.dynamic_uniforms().get("light"), Some(&light));
        assert!(cube.get_mesh().is_some());
        let material = scene.get_material(cube.get_material().unwrap()).unwrap();
        assert!(material.get_texture().is_some());
    }

    #[test]
    fn globe_demo_moves_and_spins_the_globe() {
        let mut demo = demo(5);
        let scene = demo.scene();

        let globe = scene.get_node(scene.find_node("Globe").unwrap()).unwrap();
        let mesh = scene.get_mesh(globe.get_mesh().unwrap()).unwrap();
        assert_eq!(mesh.gen_num_vertexes(), 97 * 49);
        let material = scene.get_material(globe.get_material().unwrap()).unwrap();
        let world_map = scene.find_atlas("World map").unwrap();
        assert_eq!(material.get_texture().unwrap().get_atlas(), world_map);
        let atlas = scene.get_atlas(world_map).unwrap();
        assert_eq!((atlas.get_width(), atlas.get_height()), (2048, 1024));

        let transforms = |demo: &mut Box<dyn Demo>| {
            let scene = demo.scene();
            let node = |name| scene.get_node(scene.find_node(name).unwrap()).unwrap();
            [
                node("Orbit").get_transform(),
                node("Globe").get_transform(),
                node("Moon orbit").get_transform(),
            ]
        };
        demo.update(1.0).unwrap();
        let first = transforms(&mut demo);
        demo.update(2.0).unwrap();
        let second = transforms(&mut demo);
        assert_ne!(first[0], second[0], "the globe doesn't move");
        assert_ne!(first[1], second[1], "the globe doesn't spin");
        assert_ne!(first[2], second[2], "the moon doesn't go around the globe");

        let scene = demo.scene();
        scene.update_nodes();
        let globe = scene.get_node(scene.find_node("Globe").unwrap()).unwrap();
        assert!(globe.dynamic_uniforms().get("light").is_some());
        assert!(globe.dynamic_uniforms().get("lightIntensity").is_some());

        // The moon follows the globe's path and stays at the same distance from it.
        let moon = scene.get_node(scene.find_node("Moon").unwrap()).unwrap();
        assert_eq!(moon.dynamic_uniforms().get("light"), globe.dynamic_uniforms().get("light"));
        let centre = |node: &feather::node::Node| {
            let m = node.get_global_transform().unwrap();
            feather::math::Vec3::new(m.w.x, m.w.y, m.w.z)
        };
        use cgmath::InnerSpace;
        let distance = (centre(moon) - centre(globe)).magnitude();
        assert!((distance - 2.1).abs() < 1e-4, "{distance}");
        let moon_material = scene.get_material(moon.get_material().unwrap()).unwrap();
        assert_eq!(
            moon_material.get_texture().unwrap().get_atlas(),
            scene.find_atlas("Moon map").unwrap()
        );
    }

    #[test]
    fn bouncing_balls_have_no_textures_and_their_own_colours() {
        let mut demo = demo(6);
        let scene = demo.scene();

        let floor = scene.get_node(scene.find_node("Floor").unwrap()).unwrap();
        let balls = scene.get_node(scene.find_node("Balls").unwrap()).unwrap();
        assert_eq!(balls.get_children().len(), 12);

        let mut colours = vec![format!("{:?}", floor.uniforms().get("color").unwrap())];
        for &ball in balls.get_children() {
            let ball = scene.get_node(ball).unwrap();
            assert!(ball.get_mesh().is_some());
            colours.push(format!("{:?}", ball.uniforms().get("color").unwrap()));
        }
        colours.sort();
        colours.dedup();
        assert_eq!(colours.len(), 13, "colours are not all different");
        assert!(scene.materials.iter().all(|m| m.get_texture().is_none()));

        // Only the shadows are transparent, and their material lets the floor show through.
        assert!(!floor.is_transparent());
        let shadows = scene.get_node(scene.find_node("Shadows").unwrap()).unwrap();
        assert_eq!(shadows.get_children().len(), 12);
        for &shadow in shadows.get_children() {
            let shadow = scene.get_node(shadow).unwrap();
            assert!(shadow.is_transparent());
            let material = scene.get_material(shadow.get_material().unwrap()).unwrap();
            let opacity = material.uniforms().get("opacity").unwrap();
            assert!(matches!(opacity, feather::uniform::UniformValue::Float(o) if *o < 1.0));
        }
        for &ball in balls.get_children() {
            assert!(!scene.get_node(ball).unwrap().is_transparent());
        }
        assert_eq!(scene.atlases.iter().count(), 0);

        // The balls move.
        let first = scene.get_node(*balls.get_children().iter().next().unwrap()).unwrap();
        let (handle, before) = (first.get_handle(), first.get_transform());
        demo.update(0.5).unwrap();
        assert_ne!(demo.scene().get_node(handle).unwrap().get_transform(), before);
    }

    #[test]
    fn vases_have_three_shapes_own_colours_and_some_are_transparent() {
        let mut demo = demo(7);
        let scene = demo.scene();

        let vases = scene.get_node(scene.find_node("Vases").unwrap()).unwrap();
        assert_eq!(vases.get_children().len(), 12);
        let (mut meshes, mut colours, mut scales) = (Vec::new(), Vec::new(), Vec::new());
        let mut transparent = 0;
        for &vase in vases.get_children() {
            let vase = scene.get_node(vase).unwrap();
            meshes.push(vase.get_mesh().unwrap().to_string());
            colours.push(format!("{:?}", vase.uniforms().get("color").unwrap()));
            scales.push(format!("{:.3}", vase.get_transform().x.x.hypot(vase.get_transform().x.y)));
            let material = scene.get_material(vase.get_material().unwrap()).unwrap();
            let glass = material.get_name() == Some("Glass".to_string());
            assert_eq!(vase.is_transparent(), glass, "{:?}", vase.get_name());
            transparent += vase.is_transparent() as usize;
        }
        for list in [&mut meshes, &mut colours, &mut scales] {
            list.sort();
            list.dedup();
        }
        assert_eq!(meshes.len(), 3, "not 3 shapes");
        assert_eq!(colours.len(), 12, "colours are not all different");
        assert!(scales.len() >= 6, "too few sizes: {scales:?}");
        assert!((3..=9).contains(&transparent), "{transparent} transparent vases");

        // Each shape is a closed vase of height 1 standing on z = 0.
        for name in ["Classic", "Bottle", "Urn"] {
            let mesh = scene.get_mesh(scene.find_mesh(name).unwrap()).unwrap();
            assert!(mesh.gen_num_indexes() > 1000, "{name}");
        }

        // The scene turns.
        let turntable = scene.find_node("Turntable").unwrap();
        demo.update(2.0).unwrap();
        let turned = demo.scene().get_node(turntable).unwrap().get_transform();
        assert_ne!(turned, <feather::math::Mat4 as cgmath::SquareMatrix>::identity());
    }

    /// Checks that `Plates` holds a plate per texture of atlas `Plates`, each with its own
    /// material showing its own texture, drawn with `shader`.
    fn check_plates(scene: &Scene, shader: &str) {

        let atlas_handle = scene.find_atlas("Plates").unwrap();
        let atlas = scene.get_atlas(atlas_handle).unwrap();
        let names = atlas.get_texture_names();
        assert_eq!(names.len(), 16, "{names:?}");

        // Every plate hangs from `Plates`, with its own material showing its own texture.
        let plates = scene.find_node("Plates").unwrap();
        assert_eq!(scene.get_node(plates).unwrap().get_children().len(), 16);
        let mut rects = Vec::new();
        for name in &names {
            let node = scene.get_node(scene.find_node(name).unwrap()).unwrap();
            assert_eq!(node.get_parent(), Some(plates));
            assert!(node.get_mesh().is_some());
            let material = scene.get_material(node.get_material().unwrap()).unwrap();
            let texture = *material.get_texture().unwrap();
            assert_eq!(texture.get_atlas(), atlas_handle);
            assert_eq!(Some(texture), atlas.get_texture(name));
            assert!(texture.get_u_min() < texture.get_u_max());
            assert!(texture.get_v_min() < texture.get_v_max());
            rects.push(format!("{texture:?}"));
        }
        rects.sort();
        rects.dedup();
        assert_eq!(rects.len(), 16);

        let shader = scene.get_shader(scene.find_shader(shader).unwrap()).unwrap();
        assert_eq!(shader.get_uniform_umin_name(), Some("umin"));
        assert_eq!(shader.get_uniform_vmax_name(), Some("vmax"));
    }

    #[test]
    fn atlas_demo_has_a_plate_per_atlas_texture() {
        let mut demo = demo(2);
        let scene = demo.scene();
        check_plates(scene, "Textured");

        // Nothing moves.
        let plate = scene.find_node("plate_07").unwrap();
        let before = scene.get_node(plate).unwrap().get_transform();
        demo.update(3.0).unwrap();
        assert_eq!(demo.scene().get_node(plate).unwrap().get_transform(), before);
    }

    #[test]
    fn waving_atlas_demo_has_a_plate_per_atlas_texture() {
        let mut demo = demo(3);
        let scene = demo.scene();
        check_plates(scene, "Waving");
        for name in scene.get_atlas(scene.find_atlas("Plates").unwrap()).unwrap().get_texture_names() {
            let node = scene.get_node(scene.find_node(name).unwrap()).unwrap();
            for uniform in ["phase", "speed", "strength"] {
                assert!(node.uniforms().contains(uniform), "{name}: {uniform}");
            }
        }
    }

    #[test]
    fn waving_atlas_demo_passes_the_time_to_the_plates() {
        let mut demo = demo(3);
        demo.update(2.5).unwrap();
        let scene = demo.scene();
        scene.update_nodes();
        let plate = scene.get_node(scene.find_node("plate_07").unwrap()).unwrap();
        assert_eq!(
            plate.dynamic_uniforms().get("time"),
            Some(&feather::uniform::UniformValue::Float(2.5))
        );
    }
}
