use anyhow::{anyhow, Result};

use feather::assetsprovider::AssetsProvider;
use feather::camera::Camera;
use feather::math::{Mat4, Vec3, Vec4};
use feather::orthographiccamera::{OrthographicCamera, ScreenOrigin, ScreenUnits};
use feather::scene::Scene;
use feather::text::{create_text_material, HorizontalAlign, Text, TextBuilder, VerticalAlign};

use super::demo_000_viking_room::VikingRoomDemo;
use super::testapp::Demo;

/// The overlay's height in its units: it is laid out as if the window were this many pixels
/// high, whatever its real size and the display scaling.
const OVERLAY_HEIGHT: f32 = 768.0;
/// How often the frame rate shown is updated, in seconds.
const FPS_PERIOD: f32 = 0.5;

/// Demo 12: two scenes drawn together. Behind, demo 0's spinning viking room, a 3D scene seen
/// by a perspective camera. In front, a 2D overlay seen by a flat (orthographic) camera: a
/// panel with text, the room's texture as an image with a caption, and the frame rate.
///
/// The overlay is `demo_012_overlay_scene.json`: its font (Segoe UI Semibold, rendered during
/// the build from `atlases/demo_012_overlay_atlas.font`), demo 0's texture as an image, a
/// square mesh for panels and images, three shaders (text, image, panel) and the nodes. It is
/// laid out in units of a 768-pixel-high window with the origin at the top left, so y goes
/// down to -768; the `Right` group is moved to the right edge, which depends on the window's
/// width, every frame. Panels are transparent and a little behind (z = -1) what they hold, so
/// they are drawn first.
pub struct OverlayDemo {
    room: VikingRoomDemo,
    overlay: Scene,
    overlay_camera: OrthographicCamera,
    right: usize,
    fps: Text,
    /// The start of the current frame rate period, and the frames counted since.
    fps_start: Option<f32>,
    frames: u32,
}

impl OverlayDemo {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        let room = VikingRoomDemo::new(assets)?;

        let mut overlay = Scene::load(assets, "demo_012_overlay_scene.json")?;
        let find = |what: &str, handle: Option<usize>| {
            handle.ok_or_else(|| anyhow!("the overlay has no {what}"))
        };
        let font = find("atlas `Font`", overlay.find_atlas("Font"))?;
        let shader = find("shader `Text`", overlay.find_shader("Text"))?;
        let info = find("node `Info`", overlay.find_node("Info"))?;
        let caption = find("node `Caption`", overlay.find_node("Caption"))?;
        let bottom = find("node `Bottom`", overlay.find_node("Bottom"))?;
        let right = find("node `Right`", overlay.find_node("Right"))?;
        let material = create_text_material(&mut overlay, Some("Text".into()), shader, font)?;

        let white = Vec4::new(1.0, 1.0, 1.0, 1.0);
        let grey = Vec4::new(0.78, 0.82, 0.9, 1.0);
        let top_left = |height| {
            TextBuilder::new(font)
                .height(height)
                .align(HorizontalAlign::Left, VerticalAlign::Top)
        };
        let mut add = |parent, builder, text: &str, position: (f32, f32), color| -> Result<Text> {
            let text = Text::create(&mut overlay, parent, None, material, builder, text)?;
            let node = overlay.get_node_mut(text.get_node()).unwrap();
            node.set_transform(Mat4::from_translation(Vec3::new(
                position.0, position.1, 0.0,
            )));
            node.uniforms_mut().set("color", color);
            Ok(text)
        };
        add(info, top_left(46.0), "Two scenes", (0.0, 0.0), white)?;
        add(
            info,
            top_left(24.0).line_spacing(1.15),
            "Behind: the viking room, a 3D scene seen\nby a perspective camera.\n\
             In front: this overlay, a 2D scene seen\nby a orthographic camera.",
            (0.0, -62.0),
            grey,
        )?;
        add(
            caption,
            TextBuilder::new(font)
                .height(22.0)
                .align(HorizontalAlign::Center, VerticalAlign::Top),
            "The room's texture",
            (0.0, 0.0),
            grey,
        )?;
        let fps = add(
            bottom,
            TextBuilder::new(font)
                .height(26.0)
                .align(HorizontalAlign::Left, VerticalAlign::Bottom),
            "",
            (20.0, -OVERLAY_HEIGHT + 16.0),
            Vec4::new(0.55, 0.95, 0.6, 1.0),
        )?;

        Ok(Self {
            room,
            overlay,
            overlay_camera: OrthographicCamera::new(
                ScreenOrigin::TopLeft,
                ScreenUnits::Height(OVERLAY_HEIGHT),
            ),
            right,
            fps,
            fps_start: None,
            frames: 0,
        })
    }
}

impl Demo for OverlayDemo {
    fn scene(&mut self) -> &mut Scene {
        self.room.scene()
    }

    fn camera(&mut self) -> &mut dyn Camera {
        self.room.camera()
    }

    fn scene_count(&self) -> usize {
        2
    }

    fn scene_at(&mut self, index: usize) -> &mut Scene {
        match index {
            0 => self.room.scene(),
            _ => &mut self.overlay,
        }
    }

    fn camera_at(&mut self, index: usize) -> &mut dyn Camera {
        match index {
            0 => self.room.camera(),
            _ => &mut self.overlay_camera,
        }
    }

    fn update(&mut self, time: f32) -> Result<()> {
        self.room.update(time)?;

        // The right-hand group sits at the right edge of the window.
        let (width, _) = self.overlay_camera.get_visible_size();
        self.overlay
            .get_node_mut(self.right)
            .unwrap()
            .set_transform(Mat4::from_translation(Vec3::new(width - 20.0, 0.0, 0.0)));

        // The frame rate, counted over half a second.
        self.frames += 1;
        let start = *self.fps_start.get_or_insert(time);
        if time - start >= FPS_PERIOD {
            let fps = self.frames as f32 / (time - start);
            self.fps
                .set_text(&mut self.overlay, &format!("{fps:.0} fps"))?;
            self.fps_start = Some(time);
            self.frames = 0;
        }
        Ok(())
    }
}
