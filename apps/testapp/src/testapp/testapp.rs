use anyhow::{anyhow, Result};
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

#[cfg(windows)]
use feather::assetsprovider::create_assets_provider;
use feather::assetsprovider::AssetsProvider;
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
use super::demo_012_overlay::OverlayDemo;
use super::demo_013_simple_shadow::SimpleShadowDemo;
use super::demo_014_shadows::ShadowsDemo;
use super::demo_015_city::CityDemo;
use super::demo_016_city::CityAssetsDemo;
use super::demo_017_bouncing_balls::BouncingBallsShadowDemo;
use super::demo_018_vases::VasesShadowDemo;
use super::demo_019_walker::WalkerShadowDemo;
use super::demo_020_spider::SpiderShadowDemo;
use super::demo_021_animation::AnimationDemo;
use super::demo_022_scrolling::ScrollingDemo;
use super::testapp_hud::{HudAction, TestHud};

/// One demo of the test app: a scene, seen through its active camera, and its animation. A
/// demo may have more scenes, each with its own cameras, drawn one over the other
/// (`scene_count`).
pub trait Demo {
    /// The (first) scene.
    fn scene(&mut self) -> &mut Scene;
    /// How many scenes the demo draws: scene 0 first, every next one over the previous ones.
    fn scene_count(&self) -> usize {
        1
    }
    /// Scene `index` (`0..scene_count()`).
    fn scene_at(&mut self, _index: usize) -> &mut Scene {
        self.scene()
    }
    /// Called every frame; `time` is in seconds since the demo started.
    fn update(&mut self, time: f32) -> Result<()>;

    /// What the demo shows now, for demos that switch between views (e.g. the camera's name):
    /// the HUD shows it after the demo's number and name.
    fn caption(&self) -> Option<&str> {
        None
    }

    /// The size of the window's drawing area in pixels: when the demo starts (once the window
    /// exists) and whenever it changes.
    fn on_resized(&mut self, _width: u32, _height: u32) -> Result<()> {
        Ok(())
    }

    /// The mouse, when the app's HUD doesn't take it (see `FeatherApp::on_mouse_moved` and
    /// the others for the arguments).
    fn on_mouse_moved(&mut self, _x: f32, _y: f32) -> Result<()> {
        Ok(())
    }
    fn on_mouse_left(&mut self) -> Result<()> {
        Ok(())
    }
    fn on_mouse_button(&mut self, _button: MouseButton, _pressed: bool, _x: f32, _y: f32) -> Result<()> {
        Ok(())
    }
    fn on_mouse_wheel(&mut self, _delta_x: f32, _delta_y: f32, _x: f32, _y: f32) -> Result<()> {
        Ok(())
    }
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
    DemoInfo {
        name: "Overlay",
        create: |assets| Ok(Box::new(OverlayDemo::new(assets)?)),
    },
    DemoInfo {
        name: "Simple shadow",
        create: |assets| Ok(Box::new(SimpleShadowDemo::new(assets)?)),
    },
    DemoInfo {
        name: "Shadows",
        create: |assets| Ok(Box::new(ShadowsDemo::new(assets)?)),
    },
    DemoInfo {
        name: "City",
        create: |assets| Ok(Box::new(CityDemo::new(assets)?)),
    },
    DemoInfo {
        name: "City (assets)",
        create: |assets| Ok(Box::new(CityAssetsDemo::new(assets)?)),
    },
    DemoInfo {
        name: "Bouncing balls (shadows)",
        create: |assets| Ok(Box::new(BouncingBallsShadowDemo::new(assets)?)),
    },
    DemoInfo {
        name: "Vases (shadows)",
        create: |assets| Ok(Box::new(VasesShadowDemo::new(assets)?)),
    },
    DemoInfo {
        name: "Walker (shadows)",
        create: |assets| Ok(Box::new(WalkerShadowDemo::new(assets)?)),
    },
    DemoInfo {
        name: "Spider (shadows)",
        create: |assets| Ok(Box::new(SpiderShadowDemo::new(assets)?)),
    },
    DemoInfo {
        name: "Animation",
        create: |assets| Ok(Box::new(AnimationDemo::new(assets)?)),
    },
    DemoInfo {
        name: "Scrolling",
        create: |assets| Ok(Box::new(ScrollingDemo::new(assets)?)),
    },
];

/// The development app: shows one demo at a time. The left and right arrow keys, and the
/// buttons on the left and right edges of the window, switch to the previous and next demo,
/// wrapping around. Over every demo, a HUD (`TestHud`) shows the demo's number and name, the
/// frame rate and those buttons. The mouse goes to the HUD first, and to the demo when the HUD
/// doesn't take it.
pub struct TestApp {
    assets: Box<dyn AssetsProvider>,
    demo_index: usize,
    demo: Box<dyn Demo>,
    hud: TestHud,
    /// The size of the window's drawing area, once known (`on_resized`).
    window: Option<(u32, u32)>,
    /// App time (as passed to `on_update`) when the current demo started; `None` until its
    /// first update.
    demo_start: Option<f32>,
    /// The title the HUD shows: the demo's label and caption.
    title: String,
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
        self.demo.update(time - start)?;
        self.update_title();
        self.hud.update(time)
    }

    fn on_destroy(&mut self) {
        log::trace!("on_destroy called");
    }

    /// The demo's scenes, then the HUD.
    fn get_num_scenes_to_render(&self) -> usize {
        self.demo.scene_count() + 1
    }

    fn get_scene_to_render(&mut self, scene_index: usize) -> &mut Scene {
        if scene_index < self.demo.scene_count() {
            self.demo.scene_at(scene_index)
        } else {
            self.hud.scene()
        }
    }

    fn on_key_pressed(&mut self, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::ArrowRight => self.show_next_demo(),
            KeyCode::ArrowLeft => self.show_previous_demo(),
            _ => Ok(()),
        }
    }

    fn on_resized(&mut self, width: u32, height: u32) -> Result<()> {
        self.window = Some((width, height));
        self.hud.set_window_size(width, height);
        self.demo.on_resized(width, height)
    }

    fn on_mouse_moved(&mut self, x: f32, y: f32) -> Result<()> {
        if self.hud.mouse_move(x, y)? {
            return Ok(());
        }
        self.demo.on_mouse_moved(x, y)
    }

    fn on_mouse_left(&mut self) -> Result<()> {
        self.hud.mouse_left()?;
        self.demo.on_mouse_left()
    }

    /// The left button goes to the HUD first; the others, and what the HUD doesn't take, to
    /// the demo. Then the HUD's buttons switch demos.
    fn on_mouse_button(&mut self, button: MouseButton, pressed: bool, x: f32, y: f32) -> Result<()> {
        let taken = button == MouseButton::Left && self.hud.mouse_button(pressed, x, y)?;
        if !taken {
            self.demo.on_mouse_button(button, pressed, x, y)?;
            self.update_title();
        }
        for action in self.hud.take_actions() {
            match action {
                HudAction::PreviousDemo => self.show_previous_demo()?,
                HudAction::NextDemo => self.show_next_demo()?,
            }
        }
        Ok(())
    }

    fn on_mouse_wheel(&mut self, delta_x: f32, delta_y: f32, x: f32, y: f32) -> Result<()> {
        if self.hud.mouse_wheel(delta_x, delta_y, x, y)? {
            return Ok(());
        }
        self.demo.on_mouse_wheel(delta_x, delta_y, x, y)
    }

    fn window_title(&self) -> Option<String> {
        Some(demo_window_title(self.demo_index))
    }
}

/// The window title for demo `index`: the app name, then the demo's number and name.
pub fn demo_window_title(index: usize) -> String {
    format!("Feather development app - {}", demo_label(index))
}

/// Demo `index`'s number and name, e.g. "21 Animation".
pub fn demo_label(index: usize) -> String {
    format!("{index} {}", DEMOS[index].name)
}

impl TestApp {
    /// Creates the app showing demo `demo_index` (see `DEMOS`).
    #[cfg(windows)]
    pub fn new(demo_index: usize) -> Result<Self> {
        let assets = create_assets_provider("assets")?;
        Self::with_assets(assets, demo_index)
    }

    /// `new`, with the assets from `assets`.
    pub fn with_assets(assets: Box<dyn AssetsProvider>, demo_index: usize) -> Result<Self> {
        let demo = create_demo(&*assets, demo_index)?;
        let hud = TestHud::new(&*assets)?;
        let mut app = Self {
            assets,
            demo_index,
            demo,
            hud,
            window: None,
            demo_start: None,
            title: String::new(),
        };
        app.update_title();
        Ok(app)
    }

    /// Shows the demo's label and caption in the HUD, when they changed.
    fn update_title(&mut self) {
        let label = demo_label(self.demo_index);
        let title = match self.demo.caption() {
            Some(caption) => format!("{label} - {caption}"),
            None => label,
        };
        if title != self.title {
            self.hud.set_title(&title);
            self.title = title;
        }
    }

    /// Replaces the current demo with demo `index`. If it can't be created, the current demo
    /// stays and the error is returned.
    fn show_demo(&mut self, index: usize) -> Result<()> {
        let mut demo = create_demo(&*self.assets, index)?;
        if let Some((width, height)) = self.window {
            demo.on_resized(width, height)?;
        }
        self.demo = demo;
        self.demo_index = index;
        self.demo_start = None;
        self.update_title();
        Ok(())
    }

    /// Shows the next demo; after the last comes the first.
    fn show_next_demo(&mut self) -> Result<()> {
        self.show_demo((self.demo_index + 1) % DEMOS.len())
    }

    /// Shows the previous demo; before the first comes the last.
    fn show_previous_demo(&mut self) -> Result<()> {
        self.show_demo((self.demo_index + DEMOS.len() - 1) % DEMOS.len())
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

    /// The assets the app gets at runtime: `resources/` plus the shaders, atlases and meshes
    /// built by `build.rs`.
    struct SourceAssets;

    impl AssetsProvider for SourceAssets {
        fn get_asset(&self, name: &str) -> Result<Vec<u8>> {
            let dirs = [
                concat!(env!("CARGO_MANIFEST_DIR"), "/resources"),
                concat!(env!("OUT_DIR"), "/shaders"),
                concat!(env!("OUT_DIR"), "/atlases"),
                concat!(env!("OUT_DIR"), "/meshes"),
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
    fn every_scene_has_a_camera() {
        for index in 0..DEMOS.len() {
            let mut demo = demo(index);
            for scene in 0..demo.scene_count() {
                assert!(
                    demo.scene_at(scene).active_camera().is_some(),
                    "demo {index}, scene {scene}"
                );
            }
        }
        let mut hud = TestHud::new(&SourceAssets).unwrap();
        assert!(hud.scene().active_camera().is_some());
    }

    #[test]
    fn window_title_names_the_demo() {
        assert_eq!(demo_window_title(0), "Feather development app - 0 Viking room");
        assert_eq!(
            demo_window_title(6),
            "Feather development app - 6 Bouncing balls"
        );
        assert_eq!(
            demo_window_title(17),
            "Feather development app - 17 Bouncing balls (shadows)"
        );
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

        // Ten minutes, two updates a second (advancing animations and building mesh buffers as
        // the engine would).
        let mut most = (0, 0, 0);
        for step in 1..=1200 {
            let time = step as f32 * 0.5;
            demo.update(time).unwrap();
            let scene = demo.scene();
            scene.advance_animations_to(time);
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
        // One animation per episode on screen.
        assert_eq!(demo.scene().animation_count(), names.len());
    }

    #[test]
    fn overlay_demo_draws_two_scenes_with_their_own_cameras() {
        let mut demo = demo(12);
        assert_eq!(demo.scene_count(), 2);
        let (room, overlay) = (demo.scene_at(0).get_id(), demo.scene_at(1).get_id());
        assert_ne!(room, overlay);
        assert!(demo.scene_at(0).find_node("Room").is_some());

        // The overlay's flat camera: no perspective, the view is the identity.
        let camera = demo.scene_at(1).active_camera_mut().unwrap();
        camera.set_screen_dimention(1024, 768);
        assert_eq!(camera.get_view(), <Mat4 as SquareMatrix>::identity());

        // The right-hand group follows the window's width.
        demo.update(0.0).unwrap();
        let right = demo.scene_at(1).find_node("Right").unwrap();
        let x = |demo: &mut Box<dyn Demo>| demo.scene_at(1).get_node(right).unwrap().get_transform().w.x;
        assert_eq!(x(&mut demo), 1024.0 - 20.0);
        demo.scene_at(1).active_camera_mut().unwrap().set_screen_dimention(1536, 768);
        demo.update(0.1).unwrap();
        assert_eq!(x(&mut demo), 1536.0 - 20.0);

        // Scene 0 is demo 0's room, spinning.
        let room_node = demo.scene_at(0).find_node("Room").unwrap();
        let before = demo.scene_at(0).get_node(room_node).unwrap().get_transform();
        demo.update(5.0).unwrap();
        assert_ne!(demo.scene_at(0).get_node(room_node).unwrap().get_transform(), before);
    }

    #[test]
    fn overlay_demo_slides_and_fades_its_panels_in() {
        use super::super::demo_012_overlay::{
            INTRO_DURATION, INTRO_OFFSET, INTRO_RIGHT_DELAY,
        };
        let mut demo = demo(12);
        let overlay = demo.scene_at(1);
        let node = |scene: &Scene, name: &str| scene.find_node(name).unwrap();
        let (left, right) = (node(overlay, "Left"), node(overlay, "Right slide"));
        let (panel, image) = (node(overlay, "Info panel"), node(overlay, "Image"));
        let x = |scene: &Scene, n| scene.get_node(n).unwrap().get_transform().w.x;
        let alpha = |scene: &Scene| match scene.get_node(panel).unwrap().uniforms().get("color") {
            Some(feather::uniform::UniformValue::Vec4(c)) => c.w,
            other => panic!("{other:?}"),
        };
        let opacity = |scene: &Scene| match scene.get_node(image).unwrap().uniforms().get("opacity") {
            Some(feather::uniform::UniformValue::Float(o)) => *o,
            other => panic!("{other:?}"),
        };

        // Both sides start hidden, off their places.
        overlay.advance_animations_to(0.0);
        assert_eq!(x(overlay, left), -INTRO_OFFSET);
        assert_eq!(x(overlay, right), INTRO_OFFSET);
        assert_eq!(alpha(overlay), 0.0);
        assert_eq!(opacity(overlay), 0.0);

        // Halfway through the left's intro, eased out: more than half of the way. The right
        // side hasn't started.
        overlay.advance_animations_to(INTRO_DURATION / 2.0);
        let left_x = x(overlay, left);
        assert!(-INTRO_OFFSET / 2.0 < left_x && left_x < 0.0, "{left_x}");
        assert!(alpha(overlay) > 0.72 / 2.0 && alpha(overlay) < 0.72);
        assert_eq!(x(overlay, right), INTRO_OFFSET);

        // At the end, everything is in place and as loaded, and the animations are gone.
        let mut time = INTRO_DURATION / 2.0;
        while time < INTRO_RIGHT_DELAY + INTRO_DURATION + 0.1 {
            time += 1.0 / 60.0;
            overlay.advance_animations_to(time);
        }
        assert_eq!(x(overlay, left), 0.0);
        assert_eq!(x(overlay, right), 0.0);
        assert!((alpha(overlay) - 0.72).abs() < 1e-6);
        assert_eq!(opacity(overlay), 1.0);
        assert_eq!(overlay.animation_count(), 0);
    }

    #[test]
    fn shadows_demo_has_casters_and_a_moving_sun() {
        let mut demo = demo(14);
        let scene = demo.scene();
        scene.build_missing_mesh_buffers().unwrap();
        scene.update_nodes();

        // Everything but the floor casts a shadow; the floor receives them.
        let casters: Vec<String> = scene
            .draw_items()
            .iter()
            .filter(|item| item.casts_shadow)
            .map(|item| scene.get_node(item.node).unwrap().get_name().unwrap())
            .collect();
        assert_eq!(casters.len(), 8 + 3 + 2, "{casters:?}");
        assert!(!casters.contains(&"Floor".to_string()));

        // The shader takes the light's matrix and the shadow map from the engine.
        let shader = scene.get_shader(scene.find_shader("Lit").unwrap()).unwrap();
        assert_eq!(shader.get_uniform_shadow_matrix_name(), Some("lightViewProj"));
        assert_eq!(shader.get_shadow_map_sampler_name(), Some("shadowMap"));

        // The sun moves, with the lighting's direction towards it.
        let sun = |demo: &mut Box<dyn Demo>| {
            let scene = demo.scene();
            let light = *scene.get_shadow_light().unwrap();
            let root = scene.get_node(scene.find_node("Scene root").unwrap()).unwrap();
            (light, root.uniforms().get("light").cloned())
        };
        let (before, toward_before) = sun(&mut demo);
        demo.update(3.0).unwrap();
        let (after, toward_after) = sun(&mut demo);
        assert_ne!(before.view_projection, after.view_projection);
        assert_ne!(toward_before, toward_after);
        assert_eq!(after.map_size, before.map_size);
    }

    #[test]
    fn simple_shadow_demo_has_a_figure_casting_on_a_floor() {
        let mut demo = demo(13);
        let scene = demo.scene();
        scene.build_missing_mesh_buffers().unwrap();
        scene.update_nodes();

        assert!(scene.get_shadow_light().is_some());
        let items = scene.draw_items();
        assert_eq!(items.len(), 2);
        let node = |name| scene.get_node(scene.find_node(name).unwrap()).unwrap();
        assert!(node("Human").casts_shadow());
        assert!(!node("Floor").casts_shadow());

        // The figure stands on the floor, about as tall as a person.
        let human = scene.get_mesh(node("Human").get_mesh().unwrap()).unwrap();
        let heights = human.get_vertices().iter().map(|v| v.pos.z);
        let (low, high) = heights.fold((f32::MAX, f32::MIN), |(l, h), z| (l.min(z), h.max(z)));
        assert!(low.abs() < 1e-3 && (1.6..1.9).contains(&high), "{low}..{high}");

        // Nothing moves.
        let before = node("Human").get_transform();
        demo.update(5.0).unwrap();
        let scene = demo.scene();
        assert_eq!(scene.get_node(scene.find_node("Human").unwrap()).unwrap().get_transform(), before);
    }

    #[test]
    fn city_demo_is_busy_textured_and_casts_shadows() {
        let mut demo = demo(15);
        let scene = demo.scene();
        scene.build_missing_mesh_buffers().unwrap();
        scene.update_nodes();

        // Every drawn thing but the sky is textured and casts a shadow.
        let sky = scene.find_node("Sky").unwrap();
        let items = scene.draw_items();
        assert!(items.len() > 500, "{} draws", items.len());
        for item in items.iter().filter(|i| i.node != sky) {
            let node = scene.get_node(item.node).unwrap();
            assert!(item.casts_shadow, "{:?} casts no shadow", node.get_name());
            let material = scene.get_material(item.material).unwrap();
            assert!(material.get_texture().is_some(), "{:?} has no texture", material.get_name());
        }
        assert!(scene.get_shadow_light().is_some());
        let count = |name: &str| {
            let group = scene.find_node(name).unwrap();
            scene.get_node(group).unwrap().get_children().len()
        };
        assert!(count("Cars") >= 100, "{} cars", count("Cars"));
        assert!(count("People") >= 100, "{} people", count("People"));

        // Over a few seconds, cars and people move, and so does the camera.
        let positions = |demo: &mut Box<dyn Demo>, group: &str| -> Vec<Mat4> {
            let scene = demo.scene();
            let group = scene.get_node(scene.find_node(group).unwrap()).unwrap();
            let mut nodes: Vec<usize> = group.get_children().iter().copied().collect();
            nodes.sort();
            nodes.iter().map(|&n| scene.get_node(n).unwrap().get_transform()).collect()
        };
        let (cars, people) = (positions(&mut demo, "Cars"), positions(&mut demo, "People"));
        let view = demo.scene().active_camera().unwrap().get_view();
        for step in 1..=60 {
            demo.update(step as f32 * 0.05).unwrap();
        }
        let moved = |before: &[Mat4], after: &[Mat4]| before.iter().zip(after).filter(|(a, b)| a != b).count();
        let cars_after = positions(&mut demo, "Cars");
        assert!(moved(&cars, &cars_after) > cars.len() / 2, "cars don't move");
        assert_eq!(moved(&people, &positions(&mut demo, "People")), people.len());
        assert_ne!(demo.scene().active_camera().unwrap().get_view(), view);
        // The frame rate is the test app's HUD, not a scene of the demo.
        assert_eq!(demo.scene_count(), 1);
    }

    #[test]
    fn city_demo_cuts_between_its_cameras() {
        use super::super::demo_015_city::{INTERSECTION_SHOT, STREET_SHOT};
        let mut demo = demo(15);
        let scene = demo.scene();
        let street = scene.find_camera("Street").unwrap();
        let intersection = scene.find_camera("Intersection").unwrap();
        let shadow_centre = |scene: &Scene| scene.get_shadow_light().unwrap().view_projection;

        scene.advance_animations_to(0.0);
        assert_eq!(scene.active_camera_handle(), Some(street));
        let street_shadows = shadow_centre(scene);

        // The cut to the intersection takes the shadows along.
        scene.advance_animations_to(STREET_SHOT + 1.0);
        assert_eq!(scene.active_camera_handle(), Some(intersection));
        assert_ne!(shadow_centre(scene), street_shadows);

        scene.advance_animations_to(STREET_SHOT + INTERSECTION_SHOT + 1.0);
        assert_eq!(scene.active_camera_handle(), Some(street));
    }

    #[test]
    fn city_assets_demo_switches_cameras_on_taps() {
        let mut demo = demo(16);
        demo.on_resized(1000, 800).unwrap();
        demo.update(0.5).unwrap();
        let active = |demo: &mut Box<dyn Demo>| demo.scene().active_camera_handle().unwrap();
        let camera = |demo: &mut Box<dyn Demo>, name: &str| demo.scene().find_camera(name).unwrap();
        assert_eq!(active(&mut demo), camera(&mut demo, "Street"));
        assert_eq!(demo.caption(), Some("Street"));

        let tap = |demo: &mut Box<dyn Demo>, from: (f32, f32), to: (f32, f32)| {
            demo.on_mouse_button(MouseButton::Left, true, from.0, from.1).unwrap();
            demo.on_mouse_button(MouseButton::Left, false, to.0, to.1).unwrap();
        };
        // A drag isn't a tap; a slightly moving finger is.
        tap(&mut demo, (500.0, 400.0), (700.0, 400.0));
        assert_eq!(demo.caption(), Some("Street"));
        for name in ["Crossroads", "Chase", "Above", "Street", "Crossroads"] {
            tap(&mut demo, (500.0, 400.0), (505.0, 402.0));
            assert_eq!(demo.caption(), Some(name));
            assert_eq!(active(&mut demo), camera(&mut demo, name));
        }

        // The chase camera stays behind its car, in the city, as it drives.
        tap(&mut demo, (10.0, 10.0), (10.0, 10.0));
        assert_eq!(demo.caption(), Some("Chase"));
        let mut time = 0.5;
        while time < 60.0 {
            time += 1.0 / 30.0;
            demo.update(time).unwrap();
            let scene = demo.scene();
            let eye = scene.active_camera().unwrap().get_eye_position();
            assert!(eye.x.abs().max(eye.y.abs()) < 125.0 + 20.0, "{time}: {eye:?}");
            assert!((eye.z - 4.0).abs() < 1e-3);
        }
    }

    #[test]
    fn city_assets_demo_loads_objs_and_moves() {
        let mut demo = demo(16);
        let scene = demo.scene();
        scene.build_missing_mesh_buffers().unwrap();
        scene.update_nodes();

        let sky = scene.find_node("Sky").unwrap();
        let items = scene.draw_items();
        assert!(items.len() > 500, "{} draws", items.len());
        for item in items.iter().filter(|i| i.node != sky) {
            let node = scene.get_node(item.node).unwrap();
            assert!(item.casts_shadow, "{:?} casts no shadow", node.get_name());
            let material = scene.get_material(item.material).unwrap();
            assert!(
                material.get_texture().is_some(),
                "{:?} has no texture",
                material.get_name()
            );
        }
        assert!(scene.get_shadow_light().is_some());
        let count = |name: &str| {
            let group = scene.find_node(name).unwrap();
            scene.get_node(group).unwrap().get_children().len()
        };
        assert!(count("Cars") >= 100, "{} cars", count("Cars"));
        assert!(count("People") >= 100, "{} people", count("People"));

        let positions = |demo: &mut Box<dyn Demo>, group: &str| -> Vec<Mat4> {
            let scene = demo.scene();
            let group = scene.get_node(scene.find_node(group).unwrap()).unwrap();
            let mut nodes: Vec<usize> = group.get_children().iter().copied().collect();
            nodes.sort();
            nodes
                .iter()
                .map(|&n| scene.get_node(n).unwrap().get_transform())
                .collect()
        };
        let cars = positions(&mut demo, "Cars");
        let view = demo.scene().active_camera().unwrap().get_view();
        for step in 1..=60 {
            demo.update(step as f32 * 0.05).unwrap();
        }
        let cars_after = positions(&mut demo, "Cars");
        let moved = cars
            .iter()
            .zip(&cars_after)
            .filter(|(a, b)| a != b)
            .count();
        assert!(moved > cars.len() / 2, "cars don't move");
        assert_ne!(demo.scene().active_camera().unwrap().get_view(), view);
        assert_eq!(demo.scene_count(), 1);
    }

    #[test]
    fn shadow_recreations_cast_real_shadows() {
        // Demos 17-20 are demos 6-9 with a shadow map: a light, casters, a floor that only
        // receives, and no flat shadow disc.
        for index in 17..=20 {
            let mut demo = demo(index);
            let scene = demo.scene();
            scene.build_missing_mesh_buffers().unwrap();
            scene.update_nodes();
            assert!(scene.get_shadow_light().is_some(), "demo {index}");
            assert!(scene.find_material("Shadow").is_none(), "demo {index}");

            let shader = scene.shaders.iter().next().unwrap();
            assert_eq!(shader.get_uniform_shadow_matrix_name(), Some("lightViewProj"));
            assert_eq!(shader.get_shadow_map_sampler_name(), Some("shadowMap"));

            let floor = scene.find_node("Floor").unwrap();
            let items = scene.draw_items();
            assert!(items.iter().any(|item| item.casts_shadow), "demo {index}");
            assert!(items.iter().filter(|item| item.node == floor).all(|item| !item.casts_shadow));
            assert!(items.iter().filter(|item| item.node != floor).all(|item| item.casts_shadow));
        }

        // The balls bounce, and each one casts.
        let mut balls = demo(17);
        let scene = balls.scene();
        let group = scene.get_node(scene.find_node("Balls").unwrap()).unwrap();
        assert_eq!(group.get_children().len(), 12);
        for &ball in group.get_children() {
            assert!(scene.get_node(ball).unwrap().casts_shadow());
        }
        let first = *group.get_children().iter().next().unwrap();
        let before = scene.get_node(first).unwrap().get_transform();
        balls.update(0.5).unwrap();
        assert_ne!(balls.scene().get_node(first).unwrap().get_transform(), before);

        // The vases still turn, and the glass ones stay see-through while they cast.
        let mut vases = demo(18);
        let scene = vases.scene();
        let group = scene.get_node(scene.find_node("Vases").unwrap()).unwrap();
        assert_eq!(group.get_children().len(), 12);
        let mut glass = 0;
        for &vase in group.get_children() {
            let vase = scene.get_node(vase).unwrap();
            assert!(vase.casts_shadow());
            let material = scene.get_material(vase.get_material().unwrap()).unwrap();
            if material.get_name() == Some("Glass".to_string()) {
                assert!(vase.is_transparent());
                glass += 1;
            }
        }
        assert_eq!(glass, 5);
        let turntable = scene.find_node("Turntable").unwrap();
        vases.update(2.0).unwrap();
        let turned = vases.scene().get_node(turntable).unwrap().get_transform();
        assert_ne!(turned, <Mat4 as cgmath::SquareMatrix>::identity());

        // The walker and the spider move, and nothing of them is a transparent disc.
        for index in [19, 20] {
            let mut demo = demo(index);
            let start = {
                let scene = demo.scene();
                let root = if index == 19 { "Walker" } else { "Spider" };
                scene.get_node(scene.find_node(root).unwrap()).unwrap().get_transform()
            };
            demo.update(1.0).unwrap();
            let scene = demo.scene();
            let root = if index == 19 { "Walker" } else { "Spider" };
            let later = scene.get_node(scene.find_node(root).unwrap()).unwrap().get_transform();
            assert_ne!(later, start);
            assert!(scene.nodes.iter().all(|node| !node.is_transparent()));
        }
    }

    #[test]
    fn animation_demo_moves_everything_with_animations() {
        let mut demo = demo(21);
        let scene = demo.scene();
        let lane = scene.find_node("Lane 0").unwrap();
        let x_of = |scene: &Scene, node| scene.get_node(node).unwrap().get_transform().w.x;
        assert!(scene.find_animation("Round").is_some());
        assert!(scene.find_animation("Spin").is_some());

        // The linear lane: at its start, then halfway after half the slide. The camera starts
        // far away.
        scene.advance_animations_to(0.0);
        assert!((x_of(scene, lane) + 3.0).abs() < 1e-4);
        let eye = |scene: &Scene| scene.active_camera().unwrap().get_eye_position();
        assert!((eye(scene).z - 40.0).abs() < 1e-4);
        scene.advance_animations_to(1.0);
        assert!(x_of(scene, lane).abs() < 1e-4);

        // Run long enough for several rounds and fading cubes, in frame-sized steps: the
        // animations keep going, the faders are replaced and only one exists at a time.
        let mut time = 1.0;
        while time < 30.0 {
            time += 1.0 / 30.0;
            scene.advance_animations_to(time);
        }
        let faders: Vec<String> = scene
            .nodes
            .iter()
            .filter_map(|n| n.get_name())
            .filter(|n| n.starts_with("Fader"))
            .collect();
        assert_eq!(faders.len(), 1, "{faders:?}");
        assert_ne!(faders[0], "Fader 0");
        assert!(scene.find_animation("Round").is_some());
        assert!(scene.find_animation("Spin").is_some());
        // One slide per lane, the round timer, the spinner, the fader and the camera's sway.
        assert!(scene.animation_count() <= 11 + 4, "{}", scene.animation_count());
        assert!(scene.find_animation("Sway").is_some());

        // The camera dollied in, and keeps swaying.
        assert!(scene.find_animation("Dolly").is_none());
        assert!((eye(scene).z - 21.0).abs() < 1e-4);
        let x = eye(scene).x;
        scene.advance_animations_to(time + 1.0);
        assert_ne!(eye(scene).x, x);
    }

    #[test]
    fn hud_shows_the_title_and_the_frame_rate() {
        let mut hud = TestHud::new(&SourceAssets).unwrap();
        hud.set_window_size(1024, 768);
        hud.set_title("21 Animation");
        assert_eq!(hud.get_title(), "21 Animation");
        assert_eq!(hud.get_frame_rate_text(), "-- fps");

        // 60 frames a second for one second: the text changes to "60 fps".
        for frame in 0..=60 {
            hud.update(frame as f32 / 60.0).unwrap();
        }
        assert_eq!(hud.get_frame_rate_text(), "60 fps");

        // The panel grows with a longer title.
        let panel = hud.panel();
        let width = |hud: &TestHud| hud.hud().widget(panel).unwrap().base().rect().width();
        let short = width(&hud);
        hud.set_title("16 A much longer demo name than any of the real ones");
        hud.update(1.1).unwrap();
        assert!(width(&hud) > short);
    }

    #[test]
    fn hud_buttons_sit_halfway_up_the_sides_and_the_panel_in_the_corner() {
        let mut hud = TestHud::new(&SourceAssets).unwrap();
        // Twice the size the HUD is laid out for: everything is twice as big, in the same
        // place.
        hud.set_window_size(2048, 1536);
        hud.set_title("0 Viking room");
        hud.update(0.0).unwrap();
        assert_eq!(hud.hud().size(), (1024.0, 768.0));
        let rect = |hud: &TestHud, id: feather_ui::WidgetId| hud.hud().widget(id).unwrap().base().rect();
        let (previous, next) = hud.buttons();
        let (previous, next) = (rect(&hud, previous.into()), rect(&hud, next.into()));
        let panel = rect(&hud, hud.panel().into());

        assert_eq!(previous.left, 12.0);
        assert_eq!(next.right, 1024.0 - 12.0);
        assert_eq!((previous.width(), previous.height()), (48.0, 96.0));
        // Halfway up the space above the panel.
        let middle = (panel.top - 12.0) / 2.0;
        assert_eq!((previous.top + previous.bottom) / 2.0, middle);
        assert_eq!(previous.top, next.top);
        assert_eq!((panel.left, panel.bottom), (12.0, 768.0 - 12.0));
        assert!(panel.width() > 100.0);
    }

    /// Clicks the left button at `(x, y)`, in UI units (the window is 1024 × 768, so pixels).
    fn click(app: &mut TestApp, x: f32, y: f32) {
        app.on_mouse_button(MouseButton::Left, true, x, y).unwrap();
        app.on_mouse_button(MouseButton::Left, false, x, y).unwrap();
    }

    fn button_center(app: &TestApp, button: feather_ui::WidgetId<feather_ui::Button>) -> (f32, f32) {
        let rect = app.hud.hud().widget(button).unwrap().base().rect();
        ((rect.left + rect.right) / 2.0, (rect.top + rect.bottom) / 2.0)
    }

    #[test]
    fn hud_buttons_switch_demos() {
        let mut app = TestApp::with_assets(Box::new(SourceAssets), 0).unwrap();
        app.on_resized(1024, 768).unwrap();
        app.on_update(0.0).unwrap();
        let (previous, next) = app.hud.buttons();
        let (previous, next) = (button_center(&app, previous), button_center(&app, next));

        click(&mut app, next.0, next.1);
        assert_eq!(app.demo_index, 1);
        assert_eq!(app.hud.get_title(), "1 Cuboid");
        assert_eq!(app.window_title().unwrap(), demo_window_title(1));
        click(&mut app, previous.0, previous.1);
        click(&mut app, previous.0, previous.1);
        assert_eq!(app.demo_index, DEMOS.len() - 1);
        click(&mut app, next.0, next.1);
        assert_eq!(app.demo_index, 0);

        // A click in the middle is the demo's, not the HUD's.
        click(&mut app, 512.0, 300.0);
        assert_eq!(app.demo_index, 0);
        // Pressed, then released away from the button: taken back.
        app.on_mouse_button(MouseButton::Left, true, next.0, next.1).unwrap();
        app.on_mouse_moved(512.0, 300.0).unwrap();
        app.on_mouse_button(MouseButton::Left, false, 512.0, 300.0).unwrap();
        assert_eq!(app.demo_index, 0);
        // Other buttons don't click.
        app.on_mouse_button(MouseButton::Right, true, next.0, next.1).unwrap();
        app.on_mouse_button(MouseButton::Right, false, next.0, next.1).unwrap();
        assert_eq!(app.demo_index, 0);
    }

    #[test]
    fn the_hud_is_drawn_after_the_demo_scenes() {
        let mut app = TestApp::with_assets(Box::new(SourceAssets), 12).unwrap();
        // Demo 12 has two scenes; the HUD is the third, with its own camera.
        assert_eq!(app.get_num_scenes_to_render(), 3);
        let hud = app.get_scene_to_render(2).get_id();
        assert_ne!(hud, app.get_scene_to_render(0).get_id());
        assert_ne!(hud, app.get_scene_to_render(1).get_id());
        assert!(app.get_scene_to_render(2).find_camera("UI camera").is_some());
        assert_eq!(app.hud.get_title(), "12 Overlay");

        // Switching demos keeps the HUD and changes its title.
        app.on_key_pressed(KeyCode::ArrowRight).unwrap();
        assert_eq!(app.get_num_scenes_to_render(), 2);
        assert_eq!(app.get_scene_to_render(1).get_id(), hud);
        assert_eq!(app.hud.get_title(), "13 Simple shadow");
    }

    #[test]
    fn scrolling_demo_scrolls_its_list_with_scrollbars_and_the_wheel() {
        use super::super::demo_022_scrolling::ScrollingDemo;
        use feather_ui::Widget;
        let mut demo = ScrollingDemo::new(&SourceAssets).unwrap();
        demo.on_resized(1024, 768).unwrap();
        demo.update(0.0).unwrap();
        let (list, vertical, horizontal) = demo.parts();

        // Which lines are shown, by index.
        let shown = |demo: &mut ScrollingDemo| -> Vec<usize> {
            let hud = demo.hud();
            let texts = hud.get(list).unwrap().texts();
            (0..texts.len())
                .filter(|&i| hud.scene().get_node(texts[i].get_node()).unwrap().is_visible())
                .collect()
        };

        // 80 lines don't fit, and line 8 is too wide: both scrollbars are needed.
        let hud = demo.hud();
        let view = hud.get(list).unwrap().base().rect();
        let v = hud.get(vertical).unwrap();
        assert!(v.can_scroll() && hud.get(horizontal).unwrap().can_scroll());
        assert_eq!(v.page(), view.height() as f64);
        let first = shown(&mut demo);
        assert_eq!(first[0], 0);
        assert!(first.len() > 10 && first.len() < 40, "{first:?}");

        // The wheel over the list scrolls three lines down.
        let x = (view.left + view.right) / 2.0;
        let y = (view.top + view.bottom) / 2.0;
        demo.on_mouse_wheel(0.0, -1.0, x, y).unwrap();
        demo.update(0.1).unwrap();
        assert_eq!(demo.hud().get(vertical).unwrap().value(), 66.0);
        assert_eq!(shown(&mut demo)[0], 3);

        // Dragging the vertical knob to the bottom shows the last line.
        let bar = demo.hud().get(vertical).unwrap().base().rect();
        let knob = demo.hud().get(vertical).unwrap().knob_rect();
        let (kx, ky) = ((bar.left + bar.right) / 2.0, bar.top + knob.top + 1.0);
        demo.on_mouse_button(MouseButton::Left, true, kx, ky).unwrap();
        demo.on_mouse_moved(kx, ky + 5000.0).unwrap();
        demo.on_mouse_button(MouseButton::Left, false, kx, ky + 5000.0).unwrap();
        demo.update(0.2).unwrap();
        assert_eq!(*shown(&mut demo).last().unwrap(), 79);

        // Scrolled sideways, line 8's first letters are hidden, its last ones shown.
        let max = demo.hud().get(horizontal).unwrap().max();
        demo.hud().get_mut(horizontal).unwrap().set_value(max);
        demo.hud().get_mut(vertical).unwrap().set_value(0.0);
        demo.update(0.3).unwrap();
        let hud = demo.hud();
        let glyphs = hud.get(list).unwrap().texts()[7].get_glyph_nodes().to_vec();
        let visible = |n: usize| hud.scene().get_node(n).unwrap().is_visible();
        assert!(!visible(glyphs[0]));
        assert!(visible(*glyphs.last().unwrap()));

        // A very wide window fits every line: the horizontal scrollbar goes.
        demo.on_resized(8000, 768).unwrap();
        assert!(!demo.hud().get(horizontal).unwrap().can_scroll());
        assert!(demo.hud().get(vertical).unwrap().can_scroll());
    }
}
