use anyhow::{anyhow, Result};

use feather::animation::SlideAnimation;
use feather::assetsprovider::AssetsProvider;
use feather::camera::Camera;
use feather::easing::Easing;
use feather::math::{Mat4, Vec3, Vec4};
use feather::orthographiccamera::{OrthographicCamera, ScreenOrigin, ScreenUnits};
use feather::scene::Scene;
use feather::text::{create_text_material, HorizontalAlign, Text, TextBuilder, VerticalAlign};
use feather::uniform::UniformValue;

use super::demo_000_viking_room::VikingRoomDemo;
use super::testapp::Demo;

/// The overlay's height in its units: it is laid out as if the window were this many pixels
/// high, whatever its real size and the display scaling.
const OVERLAY_HEIGHT: f32 = 768.0;

/// How long each side of the overlay takes to slide and fade in, in seconds.
pub const INTRO_DURATION: f32 = 0.8;
/// How much later than the left side the right side starts, in seconds.
pub const INTRO_RIGHT_DELAY: f32 = 0.3;
/// How far from its place, outwards, each side starts, in overlay units.
pub const INTRO_OFFSET: f32 = 120.0;

/// Demo 12: two scenes drawn together. Behind, demo 0's spinning viking room, a 3D scene seen
/// by a perspective camera. In front, a 2D overlay seen by a flat (orthographic) camera: a
/// panel with text, and the room's texture as an image with a caption. (The frame rate, in the
/// bottom left corner, is the test app's own overlay, shown over every demo.)
///
/// The overlay is `demo_012_overlay_scene.json`: its font (Segoe UI Semibold, rendered during
/// the build from `atlases/demo_012_overlay_atlas.font`), demo 0's texture as an image, a
/// square mesh for panels and images, three shaders (text, image, panel) and the nodes. It is
/// laid out in units of a 768-pixel-high window with the origin at the top left, so y goes
/// down to -768; the `Right` group is moved to the right edge, which depends on the window's
/// width, every frame. Panels are transparent and a little behind (z = -1) what they hold, so
/// they are drawn first.
///
/// The overlay slides and fades in at the start: the `Left` group from the left, then the
/// `Right slide` group (inside `Right`) from the right, each with a scene animation easing out.
/// Its step moves the group and sets the opacity of everything in it: the alpha of the panels'
/// and texts' `color`, and the image's `opacity`. The right side waits with an empty animation
/// whose `on_finish` adds its slide.
pub struct OverlayDemo {
    room: VikingRoomDemo,
    overlay: Scene,
    overlay_camera: OrthographicCamera,
    right: usize,
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
        let left = find("node `Left`", overlay.find_node("Left"))?;
        let right = find("node `Right`", overlay.find_node("Right"))?;
        let right_slide = find("node `Right slide`", overlay.find_node("Right slide"))?;
        let info_panel = find("node `Info panel`", overlay.find_node("Info panel"))?;
        let image_frame = find("node `Image frame`", overlay.find_node("Image frame"))?;
        let image = find("node `Image`", overlay.find_node("Image"))?;
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
        let title = add(info, top_left(46.0), "Two scenes", (0.0, 0.0), white)?;
        let body = add(
            info,
            top_left(24.0).line_spacing(1.15),
            "Behind: the viking room, a 3D scene seen\nby a perspective camera.\n\
             In front: this overlay, a 2D scene seen\nby a orthographic camera.",
            (0.0, -62.0),
            grey,
        )?;
        let caption_text = add(
            caption,
            TextBuilder::new(font)
                .height(22.0)
                .align(HorizontalAlign::Center, VerticalAlign::Top),
            "The room's texture",
            (0.0, 0.0),
            grey,
        )?;

        let color = |overlay: &Scene, node: usize| match overlay
            .get_node(node)
            .unwrap()
            .uniforms()
            .get("color")
        {
            Some(UniformValue::Vec4(color)) => Ok(Fade::Color(node, *color)),
            _ => Err(anyhow!("the overlay's node {node} has no `color`")),
        };
        let left_fades = vec![
            color(&overlay, info_panel)?,
            Fade::Color(title.get_node(), white),
            Fade::Color(body.get_node(), grey),
        ];
        let right_fades = vec![
            color(&overlay, image_frame)?,
            Fade::Opacity(image),
            Fade::Color(caption_text.get_node(), grey),
        ];
        add_intro(
            &mut overlay,
            "Left intro",
            left,
            -INTRO_OFFSET,
            0.0,
            left_fades,
        );
        add_intro(
            &mut overlay,
            "Right intro",
            right_slide,
            INTRO_OFFSET,
            INTRO_RIGHT_DELAY,
            right_fades,
        );

        Ok(Self {
            room,
            overlay,
            overlay_camera: OrthographicCamera::new(
                ScreenOrigin::TopLeft,
                ScreenUnits::Height(OVERLAY_HEIGHT),
            ),
            right,
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
        Ok(())
    }
}

/// Something faded in by the intro, with its look once faded in.
enum Fade {
    /// A node whose `color` uniform has this value once faded in; the intro scales its alpha.
    Color(usize, Vec4),
    /// A node with an `opacity` uniform, 1 once faded in.
    Opacity(usize),
}

/// Shows one side of the overlay `progress` of the way through its intro: `group` is moved by
/// `offset` along x at 0 and in place at 1, and what `fades` lists is invisible at 0 and as
/// loaded at 1.
fn show_intro(scene: &mut Scene, group: usize, offset: f32, fades: &[Fade], progress: f32) {
    if let Some(node) = scene.get_node_mut(group) {
        node.set_transform(Mat4::from_translation(Vec3::new(
            offset * (1.0 - progress),
            0.0,
            0.0,
        )));
    }
    let opacity = progress.clamp(0.0, 1.0);
    for fade in fades {
        match *fade {
            Fade::Color(node, color) => {
                if let Some(node) = scene.get_node_mut(node) {
                    node.uniforms_mut().set(
                        "color",
                        Vec4::new(color.x, color.y, color.z, color.w * opacity),
                    );
                }
            }
            Fade::Opacity(node) => {
                if let Some(node) = scene.get_node_mut(node) {
                    node.uniforms_mut().set("opacity", opacity);
                }
            }
        }
    }
}

/// Hides one side of the overlay and adds the animation named `name` sliding and fading it in
/// (see `show_intro`), starting `delay` seconds from now.
fn add_intro(
    scene: &mut Scene,
    name: &str,
    group: usize,
    offset: f32,
    delay: f32,
    fades: Vec<Fade>,
) {
    show_intro(scene, group, offset, &fades, 0.0);
    let intro = SlideAnimation::new(INTRO_DURATION, move |scene: &mut Scene, p| {
        show_intro(scene, group, offset, &fades, p)
    })
    .easing(Easing::CubicOut)
    .target(group);
    if delay <= 0.0 {
        scene.add_animation(Some(name.into()), intro);
        return;
    }
    let name = name.to_string();
    scene.add_animation(
        Some(format!("{name} delay")),
        SlideAnimation::new(delay, |_: &mut Scene, _| {})
            .on_finish(move |scene: &mut Scene| {
                scene.add_animation(Some(name), intro);
            })
            .target(group),
    );
}
