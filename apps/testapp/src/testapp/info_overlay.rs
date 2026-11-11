use std::ops::ControlFlow;

use anyhow::{anyhow, Result};

use feather::animation::PerpetualAnimation;
use feather::assetsprovider::AssetsProvider;
use feather::camera::Camera;
use feather::math::{Mat4, Vec3};
use feather::orthographiccamera::{OrthographicCamera, ScreenOrigin, ScreenUnits};
use feather::scene::Scene;
use feather::text::{
    create_text_material, HorizontalAlign, Text, TextBuilder, TextMethod, VerticalAlign,
};

/// The overlay's height in its units: it is laid out as if the window were this many pixels
/// high, whatever its real size and the display scaling.
const OVERLAY_HEIGHT: f32 = 768.0;
/// Distance of the panel from the window's bottom left corner, and of the texts from the
/// panel's edges.
const MARGIN: f32 = 12.0;
const PADDING: f32 = 10.0;
/// Text heights, and the space between the two lines.
const TITLE_HEIGHT: f32 = 24.0;
const FPS_HEIGHT: f32 = 20.0;
const LINE_GAP: f32 = 4.0;
/// How often the frame rate shown is updated, in seconds.
const FPS_PERIOD: f32 = 0.5;
/// The widest frame rate text the panel leaves room for.
const WIDEST_FPS: &str = "8888 fps";

/// The test app's own overlay, drawn over every demo: in the bottom left corner, on a
/// see-through panel, the demo's number and name and, below, the frame rate. Being part of the
/// picture (not the window title), it shows in full screen too.
///
/// It is a scene of its own (`testapp_overlay_scene.json`), seen by a flat camera, which the
/// app draws after the demo's scenes. It stays when demos are switched; `set_title` changes the
/// name shown.
///
/// The frame rate is a scene animation (`frame_rate_counter`): the engine advances it once per
/// frame, so it counts frames itself. Its text has a node per glyph, so changing it doesn't
/// rebuild a mesh, which would make the CPU wait for the GPU (see `doc/text-performance.md`).
pub struct InfoOverlay {
    scene: Scene,
    camera: OrthographicCamera,
    title: Text,
    panel: usize,
    /// Width of the widest frame rate text, so the panel doesn't change size with it.
    fps_width: f32,
}

impl InfoOverlay {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        let mut scene = Scene::load(assets, "testapp_overlay_scene.json")?;
        let find = |what: &str, handle: Option<usize>| {
            handle.ok_or_else(|| anyhow!("the overlay has no {what}"))
        };
        let font = find("atlas `Font`", scene.find_atlas("Font"))?;
        let shader = find("shader `Text`", scene.find_shader("Text"))?;
        let panel = find("node `Panel`", scene.find_node("Panel"))?;
        let title_anchor = find("node `Title`", scene.find_node("Title"))?;
        let fps_anchor = find("node `Frame rate`", scene.find_node("Frame rate"))?;
        let material = create_text_material(&mut scene, None, shader, font)?;

        let bottom_left = |height| {
            TextBuilder::new(font)
                .height(height)
                .align(HorizontalAlign::Left, VerticalAlign::Bottom)
        };
        let fps_builder = bottom_left(FPS_HEIGHT).method(TextMethod::NodePerGlyph);
        let fps_width = fps_builder.measure(&scene, WIDEST_FPS)?.width;

        let left = MARGIN + PADDING;
        let fps_bottom = MARGIN + PADDING;
        let title_bottom = fps_bottom + FPS_HEIGHT + LINE_GAP;
        let place = |scene: &mut Scene, node, bottom| {
            scene
                .get_node_mut(node)
                .unwrap()
                .set_transform(Mat4::from_translation(Vec3::new(left, bottom, 0.0)));
        };
        place(&mut scene, title_anchor, title_bottom);
        place(&mut scene, fps_anchor, fps_bottom);

        let title = Text::create(
            &mut scene,
            title_anchor,
            None,
            material,
            bottom_left(TITLE_HEIGHT),
            "",
        )?;
        let fps = Text::create(&mut scene, fps_anchor, None, material, fps_builder, "-- fps")?;
        scene.add_animation(Some("Frame rate".into()), frame_rate_counter(fps));

        let mut overlay = Self {
            scene,
            camera: OrthographicCamera::new(
                ScreenOrigin::BottomLeft,
                ScreenUnits::Height(OVERLAY_HEIGHT),
            ),
            title,
            panel,
            fps_width,
        };
        overlay.fit_panel();
        Ok(overlay)
    }

    /// Shows `title` (e.g. the demo's number and name) above the frame rate.
    pub fn set_title(&mut self, title: &str) -> Result<()> {
        self.title.set_text(&mut self.scene, title)?;
        self.fit_panel();
        Ok(())
    }

    pub fn get_title(&self) -> &str {
        self.title.get_text()
    }

    pub fn scene(&mut self) -> &mut Scene {
        &mut self.scene
    }

    pub fn camera(&mut self) -> &mut dyn Camera {
        &mut self.camera
    }

    /// Sizes the panel around both lines. It is a little behind them (z = -1), and transparent,
    /// so it is drawn first.
    fn fit_panel(&mut self) {
        let width = self.title.get_size().width.max(self.fps_width) + 2.0 * PADDING;
        let height = TITLE_HEIGHT + LINE_GAP + FPS_HEIGHT + 2.0 * PADDING;
        self.scene.get_node_mut(self.panel).unwrap().set_transform(
            Mat4::from_translation(Vec3::new(MARGIN, MARGIN, -1.0))
                * Mat4::from_nonuniform_scale(width, height, 1.0),
        );
    }
}

/// An animation showing the frame rate in `text`: it counts its updates (one per frame) and the
/// time they cover, and every `FPS_PERIOD` seconds shows their rate.
fn frame_rate_counter(mut text: Text) -> PerpetualAnimation<Scene> {
    let mut frames = 0u32;
    let mut seconds = 0.0f32;
    PerpetualAnimation::new(move |scene: &mut Scene, dt, _| {
        frames += 1;
        seconds += dt;
        if seconds >= FPS_PERIOD {
            let shown = format!("{:.0} fps", frames as f32 / seconds);
            if let Err(e) = text.set_text(scene, &shown) {
                log::error!("Cannot show the frame rate: {e:#}");
            }
            frames = 0;
            seconds = 0.0;
        }
        ControlFlow::Continue(())
    })
}
