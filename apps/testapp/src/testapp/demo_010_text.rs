use std::f32::consts::TAU;

use anyhow::{anyhow, Result};
use cgmath::SquareMatrix;

use feather::assetsprovider::AssetsProvider;
use feather::camera::Camera;
use feather::math::{Mat4, Point3, Vec3, Vec4};
use feather::perspectivecamera::PerspectiveCamera;
use feather::scene::Scene;
use feather::text::{
    create_text_material, HorizontalAlign, Text, TextBuilder, TextMethod, TextPlane, VerticalAlign,
};

use super::testapp::Demo;

/// Seconds for the turntable to turn once.
const TURN_PERIOD: f32 = 12.0;
/// Seconds for the title's colour to go round once.
const COLOR_PERIOD: f32 = 6.0;
/// Seconds for a letter of the wave to bob up and down once.
const WAVE_PERIOD: f32 = 1.6;
/// How far the wave's letters bob, up and down from their places.
const WAVE_HEIGHT: f32 = 0.1;
/// The wave's phase difference between neighbouring letters, in turns.
const WAVE_STEP: f32 = 0.07;

/// Demo 10: text. Static text in two fonts, aligned left, centred and right; letters of other
/// languages; a clock that changes every tenth of a second; a line whose letters bob in a wave,
/// each in its own colour; and text lying on a turning floor.
///
/// The fonts are atlases of character images of Segoe UI and Segoe UI Bold, 64 pixels high,
/// rendered by the font creator (`tools/font-creator`) and packed during the build, as
/// described in `atlases/demo_010_text_regular_atlas.font` and
/// `atlases/demo_010_text_bold_atlas.font`. The scene file has the atlases, the shader and
/// the `Texts` and `Turntable` nodes; this demo creates a material per font and a `Text` per
/// text, drawn with its node's (or an ancestor's) `color` uniform.
///
/// Every text is one mesh (`TextMethod::OneMesh`) except the wave, which has a node per letter
/// (`TextMethod::NodePerGlyph`): the demo moves each letter's node and gives it a colour.
pub struct TextDemo {
    scene: Scene,
    camera: PerspectiveCamera,
    title: usize,
    clock: Text,
    turntable: usize,
    /// The turntable's transform in the scene file; the demo turns it around its Y axis.
    turntable_base: Mat4,
    /// The wave's letters: their nodes and their transforms as laid out.
    wave: Vec<(usize, Mat4)>,
}

impl TextDemo {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        let mut scene = Scene::load(assets, "demo_010_text_scene.json")?;
        let find = |what: &str, handle: Option<usize>| {
            handle.ok_or_else(|| anyhow!("the scene has no {what}"))
        };
        let texts = find("node `Texts`", scene.find_node("Texts"))?;
        let turntable = find("node `Turntable`", scene.find_node("Turntable"))?;
        let regular = find("atlas `Regular`", scene.find_atlas("Regular"))?;
        let bold = find("atlas `Bold`", scene.find_atlas("Bold"))?;
        let shader = find("shader `Text`", scene.find_shader("Text"))?;
        let regular_material =
            create_text_material(&mut scene, Some("Regular".into()), shader, regular)?;
        let bold_material = create_text_material(&mut scene, Some("Bold".into()), shader, bold)?;

        use HorizontalAlign::{Center, Left, Right};
        let centred = |atlas, height| {
            TextBuilder::new(atlas)
                .height(height)
                .align(Center, VerticalAlign::Center)
        };
        // A text at `position`, optionally in its own colour.
        let add = |scene: &mut Scene,
                   name: &str,
                   bold: bool,
                   builder: TextBuilder,
                   text: &str,
                   position: (f32, f32),
                   color: Option<Vec4>|
         -> Result<Text> {
            let material = if bold {
                bold_material
            } else {
                regular_material
            };
            let text = Text::create(scene, texts, Some(name.into()), material, builder, text)?;
            let node = scene.get_node_mut(text.get_node()).unwrap();
            node.set_transform(Mat4::from_translation(Vec3::new(
                position.0, position.1, 0.0,
            )));
            if let Some(color) = color {
                node.uniforms_mut().set("color", color);
            }
            Ok(text)
        };

        let title = add(
            &mut scene,
            "Title",
            true,
            centred(bold, 1.1),
            "Text in feather.rs",
            (0.0, 3.4),
            None,
        )?;

        let column = |align| {
            TextBuilder::new(regular)
                .height(0.36)
                .line_spacing(1.1)
                .align(align, VerticalAlign::Top)
        };
        let grey = Some(Vec4::new(0.8, 0.85, 0.9, 1.0));
        add(
            &mut scene,
            "Left",
            false,
            column(Left),
            "Aligned left:\nlines start\nat the node's origin",
            (-5.4, 2.3),
            grey,
        )?;
        add(
            &mut scene,
            "Centre",
            false,
            column(Center),
            "Centred:\nlines are centred\non the node's origin",
            (0.0, 2.3),
            grey,
        )?;
        add(
            &mut scene,
            "Right",
            false,
            column(Right),
            "Aligned right:\nlines end\nat the node's origin",
            (5.4, 2.3),
            grey,
        )?;

        let gold = Some(Vec4::new(1.0, 0.82, 0.35, 1.0));
        add(
            &mut scene,
            "Polish",
            false,
            centred(regular, 0.5),
            "Zażółć gęślą jaźń",
            (0.0, 0.5),
            gold,
        )?;
        add(
            &mut scene,
            "German and Spanish",
            false,
            centred(regular, 0.5),
            "Größe · „Grüße“ · ¿Qué tal, señor?",
            (0.0, -0.1),
            gold,
        )?;

        let clock = add(
            &mut scene,
            "Clock",
            true,
            centred(bold, 0.6),
            "",
            (0.0, -1.0),
            Some(Vec4::new(0.45, 0.9, 0.55, 1.0)),
        )?;

        // A node per letter, so each can move and have its own colour.
        let wave = add(
            &mut scene,
            "Wave",
            true,
            centred(bold, 0.45).method(TextMethod::NodePerGlyph),
            "A node per letter: each one moves on its own",
            (0.0, -1.85),
            None,
        )?;
        let count = wave.get_glyph_nodes().len();
        let wave: Vec<(usize, Mat4)> = wave
            .get_glyph_nodes()
            .iter()
            .enumerate()
            .map(|(i, &glyph)| {
                // Round the colour wheel along the line.
                let hue = i as f32 / count as f32 * TAU;
                let channel = |offset: f32| 0.6 + 0.4 * (hue + offset * TAU / 3.0).cos();
                let color = Vec4::new(channel(0.0), channel(1.0), channel(2.0), 1.0);
                let node = scene.get_node_mut(glyph).unwrap();
                node.uniforms_mut().set("color", color);
                (glyph, node.get_transform())
            })
            .collect();

        // Lying on the floor, half see-through.
        let floor_text = Text::create(
            &mut scene,
            turntable,
            Some("Floor text".into()),
            bold_material,
            centred(bold, 0.7).plane(TextPlane::XZ),
            "On the floor\n— turning —",
        )?;
        scene
            .get_node_mut(floor_text.get_node())
            .unwrap()
            .uniforms_mut()
            .set("color", Vec4::new(0.45, 0.7, 1.0, 0.85));

        // Straight in front of the texts, looking level so their sides don't lean; the floor
        // text is far enough below to be seen from above.
        let mut camera = PerspectiveCamera::new();
        camera.set_fov(45.0).set_near_far(0.1, 50.0).set_view(
            Point3::new(0.0, 0.4, 11.0),
            Point3::new(0.0, 0.4, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );

        let mut demo = Self {
            scene,
            camera,
            title: title.get_node(),
            clock,
            turntable,
            turntable_base: Mat4::identity(),
            wave,
        };
        demo.turntable_base = demo.scene.get_node(turntable).unwrap().get_transform();
        demo.update(0.0)?;
        Ok(demo)
    }

    /// The clock's text at `time` seconds.
    fn clock_text(time: f32) -> String {
        format!("Time: {:.1} s", (time * 10.0).floor() / 10.0)
    }
}

impl Demo for TextDemo {
    fn scene(&mut self) -> &mut Scene {
        &mut self.scene
    }

    fn camera(&mut self) -> &mut dyn Camera {
        &mut self.camera
    }

    fn update(&mut self, time: f32) -> Result<()> {
        // Rebuilt only when the shown text changes, ten times a second.
        self.clock
            .set_text(&mut self.scene, &Self::clock_text(time))?;

        let angle = time / TURN_PERIOD * TAU;
        self.scene
            .get_node_mut(self.turntable)
            .unwrap()
            .set_transform(self.turntable_base * Mat4::from_angle_y(cgmath::Rad(angle)));

        // The title's colour goes slowly round the colour wheel.
        let phase = time / COLOR_PERIOD * TAU;
        let channel = |offset: f32| 0.65 + 0.35 * (phase + offset * TAU / 3.0).cos();
        let color = Vec4::new(channel(0.0), channel(1.0), channel(2.0), 1.0);
        self.scene
            .get_node_mut(self.title)
            .unwrap()
            .uniforms_mut()
            .set("color", color);

        for (i, &(glyph, base)) in self.wave.iter().enumerate() {
            let phase = (time / WAVE_PERIOD - i as f32 * WAVE_STEP) * TAU;
            let lift = Mat4::from_translation(Vec3::new(0.0, WAVE_HEIGHT * phase.sin(), 0.0));
            self.scene
                .get_node_mut(glyph)
                .unwrap()
                .set_transform(lift * base);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_text_shows_tenths() {
        assert_eq!(TextDemo::clock_text(0.0), "Time: 0.0 s");
        assert_eq!(TextDemo::clock_text(12.39), "Time: 12.3 s");
    }
}
