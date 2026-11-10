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
use super::demo_008_walker::WalkerDemo;
use super::demo_009_spider::SpiderDemo;
use super::demo_010_text::TextDemo;
use super::demo_011_crawl::CrawlDemo;

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
    DemoInfo {
        name: "Walker",
        create: |assets| Ok(Box::new(WalkerDemo::new(assets)?)),
    },
    DemoInfo {
        name: "Spider",
        create: |assets| Ok(Box::new(SpiderDemo::new(assets)?)),
    },
    DemoInfo {
        name: "Text",
        create: |assets| Ok(Box::new(TextDemo::new(assets)?)),
    },
    DemoInfo {
        name: "Crawl",
        create: |assets| Ok(Box::new(CrawlDemo::new(assets)?)),
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

    /// World position of the origin of node `name`, after `update_nodes`.
    fn node_position(scene: &mut Scene, name: &str) -> feather::math::Vec3 {
        scene.update_nodes();
        let node = scene.get_node(scene.find_node(name).unwrap()).unwrap();
        let m = node.get_global_transform().unwrap();
        feather::math::Vec3::new(m.w.x, m.w.y, m.w.z)
    }

    #[test]
    fn walker_is_textured_from_one_atlas_and_walks() {
        use cgmath::InnerSpace;

        let mut demo = demo(8);
        let scene = demo.scene();

        // Every body part has a texture from the `Walker` atlas.
        let atlas = scene.find_atlas("Walker").unwrap();
        let names = scene.get_atlas(atlas).unwrap().get_texture_names();
        for texture in ["face", "floor", "pants", "shadow", "shirt", "shoes", "skin"] {
            assert!(names.contains(&texture), "{texture} missing from {names:?}");
        }
        assert!(scene
            .materials
            .iter()
            .all(|m| m.get_texture().map(|t| t.get_atlas()) == Some(atlas)));

        // Standing on the floor: the feet near z = 0, the head at about 1.75.
        let head = node_position(scene, "Head");
        assert!((1.4..1.9).contains(&head.z), "{head:?}");
        let ankle = node_position(scene, "LeftAnkle");
        assert!((0.0..0.15).contains(&ankle.z), "{ankle:?}");

        // Half a cycle later the legs have swapped: the left foot was in front, now the
        // right one is.
        let forward = |demo: &mut Box<dyn Demo>, time: f32| {
            demo.update(time).unwrap();
            let scene = demo.scene();
            let hips = node_position(scene, "Hips");
            let walker = scene.get_node(scene.find_node("Walker").unwrap()).unwrap();
            let m = walker.get_transform();
            let facing = feather::math::Vec3::new(m.y.x, m.y.y, m.y.z);
            let left = (node_position(scene, "LeftAnkle") - hips).dot(facing);
            let right = (node_position(scene, "RightAnkle") - hips).dot(facing);
            (left, right)
        };
        let (left, right) = forward(&mut demo, 0.0);
        assert!(left > right + 0.2, "left {left}, right {right}");
        let (left, right) = forward(&mut demo, 0.55);
        assert!(right > left + 0.2, "left {left}, right {right}");

        // It moves along the circle, facing where it goes.
        let start = node_position(demo.scene(), "Walker");
        demo.update(1.0).unwrap();
        let later = node_position(demo.scene(), "Walker");
        demo.update(1.01).unwrap();
        let next = node_position(demo.scene(), "Walker");
        assert!((later - start).magnitude() > 0.5);
        let scene = demo.scene();
        let m = scene.get_node(scene.find_node("Walker").unwrap()).unwrap().get_transform();
        let facing = feather::math::Vec3::new(m.y.x, m.y.y, m.y.z);
        assert!((next - later).normalize().dot(facing) > 0.99);
    }

    #[test]
    fn spider_walks_with_alternating_legs() {
        use cgmath::InnerSpace;

        let mut demo = demo(9);
        {
            let scene = demo.scene();
            for leg in ["L1", "L2", "L3", "L4", "R1", "R2", "R3", "R4"] {
                for bone in ["Hip", "Femur", "Knee", "Ankle"] {
                    assert!(scene.find_node(&format!("{leg}{bone}")).is_some(), "{leg}{bone}");
                }
            }
            let atlas = scene.find_atlas("Spider").unwrap();
            assert!(scene
                .materials
                .iter()
                .all(|m| m.get_texture().map(|t| t.get_atlas()) == Some(atlas)));
        }

        // The tip of a leg: the end of its tarsus, 0.32 along the ankle bone's +X.
        let tip_height = |scene: &mut Scene, leg: &str| {
            scene.update_nodes();
            let ankle = scene.find_node(&format!("{leg}Ankle")).unwrap();
            let ankle = scene.get_node(ankle).unwrap();
            let tip = ankle.get_global_transform().unwrap()
                * feather::math::Vec4::new(0.32, 0.0, 0.0, 1.0);
            tip.z
        };
        // Through the cycle (1 s), the spider always stands on at least three feet, with
        // feet down on both sides; every leg lifts at some point, and legs that step in
        // different groups (like the two front legs) are not up together.
        let legs = ["L1", "L2", "L3", "L4", "R1", "R2", "R3", "R4"];
        let mut lifted = [false; 8];
        for step in 0..40 {
            demo.update(step as f32 * 0.025).unwrap();
            let scene = demo.scene();
            let tips: Vec<f32> = legs.iter().map(|l| tip_height(scene, l)).collect();
            let down: Vec<&str> = legs
                .iter()
                .zip(&tips)
                .filter(|(_, &z)| z < 0.06)
                .map(|(l, _)| *l)
                .collect();
            assert!(down.len() >= 3, "step {step}: only {down:?} on the floor: {tips:?}");
            assert!(down.iter().any(|l| l.starts_with('L')), "step {step}: {tips:?}");
            assert!(down.iter().any(|l| l.starts_with('R')), "step {step}: {tips:?}");
            assert!(!(tips[0] > 0.08 && tips[4] > 0.08), "step {step}: both front legs up");
            for (up, &z) in lifted.iter_mut().zip(&tips) {
                *up |= z > 0.08;
            }
            // Feet never go far into the floor.
            assert!(tips.iter().all(|&z| z > -0.05), "step {step}: {tips:?}");
        }
        assert!(lifted.iter().all(|&up| up), "not every leg lifts: {lifted:?}");

        // It moves along the circle.
        let position = |demo: &mut Box<dyn Demo>| {
            let scene = demo.scene();
            let spider = scene.find_node("Spider").unwrap();
            let m = scene.get_node(spider).unwrap().get_transform();
            feather::math::Vec3::new(m.w.x, m.w.y, m.w.z)
        };
        demo.update(0.0).unwrap();
        let start = position(&mut demo);
        demo.update(1.0).unwrap();
        assert!((position(&mut demo) - start).magnitude() > 0.5);
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

    #[test]
    fn text_demo_shows_texts_and_updates_the_clock() {
        let mut demo = demo(10);
        let scene = demo.scene();

        // Every character of the font creator's default set is in both fonts.
        for name in ["Regular", "Bold"] {
            let atlas = scene.get_atlas(scene.find_atlas(name).unwrap()).unwrap();
            for c in "Aaż’…".chars() {
                assert!(atlas.has_texture(&(c as u32).to_string()), "{name}: {c}");
            }
        }

        let texts = scene.get_node(scene.find_node("Texts").unwrap()).unwrap();
        assert_eq!(texts.get_children().len(), 8);
        let wave = scene.find_node("Wave").unwrap();
        for &text in texts.get_children().iter().filter(|&&t| t != wave) {
            let node = scene.get_node(text).unwrap();
            assert!(node.is_transparent(), "{:?}", node.get_name());
            assert!(node.get_mesh().is_some(), "{:?}", node.get_name());
        }

        // The wave has a node per letter, sharing one mesh, each in its own colour and moving.
        let letters: Vec<usize> = scene.get_node(wave).unwrap().get_children().iter().copied().collect();
        assert_eq!(letters.len(), "Anodeperletter:eachonemovesonitsown".chars().count());
        assert_eq!(scene.get_node(wave).unwrap().get_mesh(), None);
        let meshes: std::collections::HashSet<_> =
            letters.iter().map(|&l| scene.get_node(l).unwrap().get_mesh().unwrap()).collect();
        assert_eq!(meshes.len(), 1);
        let colours: std::collections::HashSet<String> = letters
            .iter()
            .map(|&l| format!("{:?}", scene.get_node(l).unwrap().uniforms().get("color")))
            .collect();
        assert!(colours.len() > 10, "{} colours", colours.len());
        let before = scene.get_node(letters[0]).unwrap().get_transform();
        demo.update(0.3).unwrap();
        assert_ne!(demo.scene().get_node(letters[0]).unwrap().get_transform(), before);
        let scene = demo.scene();
        let floor_text = scene.get_node(scene.find_node("Floor text").unwrap()).unwrap();
        let floor_mesh = scene.get_mesh(floor_text.get_mesh().unwrap()).unwrap();
        assert!(floor_mesh.get_vertices().iter().all(|v| v.pos.y == 0.0), "not in the XZ plane");

        // The clock's mesh is replaced when its text changes, and the old one removed.
        let clock = scene.find_node("Clock").unwrap();
        let mesh = |demo: &mut Box<dyn Demo>| demo.scene().get_node(clock).unwrap().get_mesh();
        let meshes = scene.meshes.iter().count();
        let first = mesh(&mut demo);
        demo.update(0.32).unwrap();
        assert_eq!(mesh(&mut demo), first);
        demo.update(1.25).unwrap();
        assert!(mesh(&mut demo).is_some());
        assert_eq!(demo.scene().meshes.iter().count(), meshes);
        let turntable = demo.scene().find_node("Turntable").unwrap();
        let turned = demo.scene().get_node(turntable).unwrap().get_transform();
        demo.update(2.0).unwrap();
        assert_ne!(demo.scene().get_node(turntable).unwrap().get_transform(), turned);
    }

    #[test]
    fn crawl_never_ends_and_cleans_up_after_itself() {
        let mut demo = demo(11);
        let episodes = |scene: &Scene| {
            let crawl = scene.get_node(scene.find_node("Crawl").unwrap()).unwrap();
            let mut names: Vec<String> = crawl
                .get_children()
                .iter()
                .map(|&e| scene.get_node(e).unwrap().get_name().unwrap())
                .collect();
            names.sort();
            names
        };
        assert_eq!(episodes(demo.scene()), ["Episode 1"]);
        let stars = demo.scene().find_node("Stars").unwrap();
        assert!(demo.scene().get_node(stars).unwrap().get_mesh().is_some());

        // Ten minutes, two updates a second (building mesh buffers as the renderer would).
        let mut most = (0, 0, 0);
        for step in 1..=1200 {
            demo.update(step as f32 * 0.5).unwrap();
            let scene = demo.scene();
            scene.build_missing_mesh_buffers().unwrap();
            let counts = (
                scene.nodes.iter().count(),
                scene.meshes.iter().count(),
                scene.buffers.iter().count(),
            );
            most = (most.0.max(counts.0), most.1.max(counts.1), most.2.max(counts.2));
        }
        let names = episodes(demo.scene());
        assert!(!names.contains(&"Episode 1".to_string()), "{names:?}");
        let newest: u32 = names
            .iter()
            .map(|n| n["Episode ".len()..].parse::<u32>().unwrap())
            .max()
            .unwrap();
        assert!(newest > 10, "only {newest} episodes in ten minutes");
        // Old episodes are removed: a few episodes' worth of objects at most.
        assert!(most.0 < 40 && most.1 < 20 && most.2 < 20, "{most:?}");
    }
}
