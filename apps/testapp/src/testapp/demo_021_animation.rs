use std::f32::consts::TAU;
use std::ops::ControlFlow;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use cgmath::{Deg, InnerSpace};

use feather::animation::{PerpetualAnimation, SlideAnimation};
use feather::animator::Animator;
use feather::assetsprovider::AssetsProvider;
use feather::camera::Camera;
use feather::easing::Easing;
use feather::math::{Mat4, Point3, Vec3};
use feather::perspectivecamera::PerspectiveCamera;
use feather::scene::Scene;
use feather::text::{create_text_material, HorizontalAlign, Text, TextBuilder, VerticalAlign};

use super::testapp::Demo;

/// Kinds of easing, one per lane, top to bottom: `Linear`, then the in/out/in-out families of
/// `Easing::ALL` (in that list's order).
const KINDS: usize = 11;
/// Y of the top lane, and the distance between lanes.
const TOP_LANE: f32 = 4.6;
const LANE_STEP: f32 = 0.9;
/// X where the cubes start and end their slides. Back easings overshoot by about 35% of the
/// slide, so the labels and the showcase keep clear of that much more on each side.
const LEFT: f32 = -3.0;
const RIGHT: f32 = 3.0;
/// X where the lane labels end.
const LABELS_END: f32 = -5.4;
/// Seconds a slide lasts, and the pause before the next round.
const SLIDE_TIME: f32 = 2.0;
const PAUSE: f32 = 0.7;
/// Where the showcase's spinning and fading cubes are.
const SPINNER_AT: Vec3 = Vec3::new(8.0, 2.6, 0.0);
const FADER_AT: Vec3 = Vec3::new(8.0, -2.2, 0.0);
/// Camera: how far it starts and ends its dolly, how long that takes, how far it sways
/// sideways and how long a sway takes.
const CAMERA_START: f32 = 40.0;
const CAMERA_END: f32 = 21.0;
const DOLLY_TIME: f32 = 3.0;
const SWAY: f32 = 2.0;
const SWAY_PERIOD: f32 = 14.0;

/// Demo 21: animations. On the left, a lane per kind of easing, each with a cube sliding from
/// one end to the other: all lanes in, then out, then in-out (named at the top), each forth and
/// back. On the right, a cube spinning forever, and a cube fading in, then out, which is
/// replaced by a new one in another colour. The camera dollies in at the start and then sways
/// from side to side.
///
/// Everything in the scene moves by scene animations (`Scene::add_animation`), which the engine
/// advances every frame:
/// - each round adds a `SlideAnimation` per lane, plus a timer slide whose `on_finish` starts
///   the next round: callbacks can add animations;
/// - the spinner is a `PerpetualAnimation`;
/// - the fader's fade-in adds the fade-out in its `on_finish`, whose own `on_finish` removes the
///   cube (the animation's own target) and creates the next one.
///
/// The camera isn't part of the scene: a standalone `Animator<CameraRig>` animates it, with a
/// slide (the dolly) and a perpetual animation (the sway) changing different parts of the rig.
///
/// The scene file has the meshes, shaders, materials, the font atlas and the `Lanes`,
/// `Showcase` and `Labels` nodes; this demo adds the rest.
pub struct AnimationDemo {
    scene: Scene,
    rig: CameraRig,
    camera_animator: Animator<CameraRig>,
}

/// What the camera animations change: the distance to the scene and the sideways offset.
pub struct CameraRig {
    camera: PerspectiveCamera,
    distance: f32,
    sway: f32,
}

impl CameraRig {
    fn apply(&mut self) {
        self.camera.set_view(
            Point3::new(1.3 + self.sway, -0.6 * self.sway, self.distance),
            Point3::new(1.3, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );
    }
}

/// The lanes' cubes and the round headers, shared by every round's callbacks.
struct Lanes {
    /// Cube node and Y of each lane, top to bottom (see `KINDS`).
    cubes: Vec<(usize, f32)>,
    /// Header nodes for the in, out and in-out rounds.
    headers: [usize; 3],
}

/// What the fader needs to create its next cube.
#[derive(Clone, Copy)]
struct Fader {
    parent: usize,
    mesh: usize,
    material: usize,
    /// How many cubes were created before this one; picks the colour.
    count: u32,
}

impl AnimationDemo {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        let mut scene = Scene::load(assets, "demo_021_animation_scene.json")?;
        let find = |what: &str, handle: Option<usize>| {
            handle.ok_or_else(|| anyhow!("the scene has no {what}"))
        };
        let lanes_node = find("node `Lanes`", scene.find_node("Lanes"))?;
        let showcase = find("node `Showcase`", scene.find_node("Showcase"))?;
        let labels = find("node `Labels`", scene.find_node("Labels"))?;
        let cube = find("mesh `Cube`", scene.find_mesh("Cube"))?;
        let track = find("mesh `Track`", scene.find_mesh("Track"))?;
        let glossy = find("material `Glossy`", scene.find_material("Glossy"))?;
        let matte = find("material `Matte`", scene.find_material("Matte"))?;
        let font = find("atlas `Font`", scene.find_atlas("Font"))?;
        let text_shader = find("shader `Text`", scene.find_shader("Text"))?;
        let text_material = create_text_material(&mut scene, None, text_shader, font)?;

        let label = |scene: &mut Scene, text: &str, at: (f32, f32), height, align| {
            let builder = TextBuilder::new(font)
                .height(height)
                .align(align, VerticalAlign::Center);
            let text = Text::create(scene, labels, None, text_material, builder, text)?;
            let node = text.get_node();
            scene
                .get_node_mut(node)
                .unwrap()
                .set_transform(Mat4::from_translation(Vec3::new(at.0, at.1, 0.0)));
            anyhow::Ok(node)
        };

        let mut cubes = Vec::with_capacity(KINDS);
        for kind in 0..KINDS {
            let y = TOP_LANE - kind as f32 * LANE_STEP;
            let name = match kind {
                0 => "Linear".to_string(),
                _ => {
                    let first = easing_for(kind, 0).name();
                    first.trim_end_matches(" in").to_string()
                }
            };
            label(&mut scene, &name, (LABELS_END, y), 0.38, HorizontalAlign::Right)?;

            let rail = scene.create_node(None, lanes_node)?;
            scene.node_set_mesh(rail, track)?;
            scene.node_set_material(rail, matte)?;
            scene
                .get_node_mut(rail)
                .unwrap()
                .set_transform(Mat4::from_translation(Vec3::new(0.0, y, 0.0)));

            let node = scene.create_node(Some(format!("Lane {kind}")), lanes_node)?;
            scene.node_set_mesh(node, cube)?;
            scene.node_set_material(node, glossy)?;
            let hue = kind as f32 / KINDS as f32;
            scene
                .get_node_mut(node)
                .unwrap()
                .uniforms_mut()
                .set("color", hsv_to_rgb(hue, 0.7, 0.95));
            cubes.push((node, y));
        }

        let header_y = TOP_LANE + 1.1;
        let mut headers = [0; 3];
        for (variant, title) in ["Ease in", "Ease out", "Ease in-out"].iter().enumerate() {
            headers[variant] =
                label(&mut scene, title, (0.0, header_y), 0.6, HorizontalAlign::Center)?;
        }

        label(
            &mut scene,
            "Perpetual",
            (SPINNER_AT.x, SPINNER_AT.y - 1.3),
            0.38,
            HorizontalAlign::Center,
        )?;
        label(
            &mut scene,
            "Fade in, fade out,\nremove, replace",
            (FADER_AT.x, FADER_AT.y - 1.6),
            0.38,
            HorizontalAlign::Center,
        )?;

        start_round(&mut scene, Arc::new(Lanes { cubes, headers }), 0);
        add_spinner(&mut scene, showcase, cube, glossy)?;
        add_fader(
            &mut scene,
            Fader {
                parent: showcase,
                mesh: cube,
                material: glossy,
                count: 0,
            },
        )?;

        let mut camera = PerspectiveCamera::new();
        camera.set_fov(45.0).set_near_far(0.1, 100.0);
        let mut rig = CameraRig {
            camera,
            distance: CAMERA_START,
            sway: 0.0,
        };
        rig.apply();

        let mut camera_animator = Animator::new();
        camera_animator.add(
            Some("Dolly".into()),
            SlideAnimation::new(DOLLY_TIME, |rig: &mut CameraRig, p| {
                rig.distance = CAMERA_START + (CAMERA_END - CAMERA_START) * p;
            })
            .easing(Easing::CubicOut),
        );
        camera_animator.add(
            Some("Sway".into()),
            PerpetualAnimation::new(|rig: &mut CameraRig, _, elapsed| {
                rig.sway = SWAY * (elapsed * TAU / SWAY_PERIOD).sin();
                ControlFlow::Continue(())
            }),
        );

        Ok(Self {
            scene,
            rig,
            camera_animator,
        })
    }
}

/// The easing of lane `kind` in rounds of `variant` (0 in, 1 out, 2 in-out).
fn easing_for(kind: usize, variant: usize) -> Easing {
    match kind {
        0 => Easing::Linear,
        _ => Easing::ALL[1 + (kind - 1) * 3 + variant],
    }
}

/// Starts round `round`: shows its header and slides every lane's cube across, then starts the
/// next round after a pause. Rounds go in, out, in-out, each forth and back.
fn start_round(scene: &mut Scene, lanes: Arc<Lanes>, round: usize) {
    let variant = (round / 2) % 3;
    let (from, to) = if round % 2 == 0 {
        (LEFT, RIGHT)
    } else {
        (RIGHT, LEFT)
    };
    for (i, &header) in lanes.headers.iter().enumerate() {
        if let Some(node) = scene.get_node_mut(header) {
            node.set_visible(i == variant);
        }
    }
    for (kind, &(cube, y)) in lanes.cubes.iter().enumerate() {
        scene.add_animation(
            None,
            SlideAnimation::new(SLIDE_TIME, move |scene: &mut Scene, p| {
                if let Some(node) = scene.get_node_mut(cube) {
                    let x = from + (to - from) * p;
                    node.set_transform(Mat4::from_translation(Vec3::new(x, y, 0.0)));
                }
            })
            .easing(easing_for(kind, variant))
            .target(cube),
        );
    }
    scene.add_animation(
        Some("Round".into()),
        SlideAnimation::new(SLIDE_TIME + PAUSE, |_: &mut Scene, _| {})
            .on_finish(move |scene: &mut Scene| start_round(scene, lanes, round + 1)),
    );
}

/// Adds the cube spinning forever.
fn add_spinner(scene: &mut Scene, parent: usize, mesh: usize, material: usize) -> Result<()> {
    let node = scene.create_node(Some("Spinner".into()), parent)?;
    scene.node_set_mesh(node, mesh)?;
    scene.node_set_material(node, material)?;
    scene
        .get_node_mut(node)
        .unwrap()
        .uniforms_mut()
        .set("color", Vec3::new(0.95, 0.75, 0.3));
    let axis = Vec3::new(1.0, 1.0, 0.3).normalize();
    scene.add_animation(
        Some("Spin".into()),
        PerpetualAnimation::new(move |scene: &mut Scene, _, elapsed| {
            if let Some(node) = scene.get_node_mut(node) {
                node.set_transform(
                    Mat4::from_translation(SPINNER_AT)
                        * Mat4::from_axis_angle(axis, Deg(elapsed * 80.0))
                        * Mat4::from_scale(3.0),
                );
            }
            ControlFlow::Continue(())
        })
        .target(node),
    );
    Ok(())
}

/// Creates a fading cube: it rises in while fading in, then fades out while shrinking, is
/// removed, and the next one (in the next colour) is created.
fn add_fader(scene: &mut Scene, fader: Fader) -> Result<()> {
    let node = scene.create_node(Some(format!("Fader {}", fader.count)), fader.parent)?;
    scene.node_set_mesh(node, fader.mesh)?;
    scene.node_set_material(node, fader.material)?;
    let hue = (fader.count as f32 * 0.29).fract();
    let n = scene.get_node_mut(node).unwrap();
    n.set_transparent(true);
    n.uniforms_mut()
        .set("color", hsv_to_rgb(hue, 0.65, 0.95))
        .set("opacity", 0.0f32);

    let place = move |scene: &mut Scene, rise: f32, scale: f32, opacity: f32| {
        if let Some(n) = scene.get_node_mut(node) {
            n.set_transform(
                Mat4::from_translation(FADER_AT + Vec3::new(0.0, rise, 0.0))
                    * Mat4::from_angle_y(Deg(35.0))
                    * Mat4::from_angle_x(Deg(25.0))
                    * Mat4::from_scale(scale),
            );
            n.uniforms_mut().set("opacity", opacity);
        }
    };

    let fade_out = SlideAnimation::new(2.0, move |scene: &mut Scene, p| {
        place(scene, 0.0, 3.0 * (1.0 - 0.6 * p), 1.0 - p)
    })
    // Stays nearly solid for a while, then fades quickly.
    .easing(Easing::ExponentialIn)
    .on_finish(move |scene: &mut Scene| {
        // Removing the node also cancels animations targeting it, including this one.
        let _ = scene.remove_node(node);
        let next = Fader {
            count: fader.count + 1,
            ..fader
        };
        if let Err(e) = add_fader(scene, next) {
            log::error!("Cannot add the next fading cube: {e:#}");
        }
    })
    .target(node);

    scene.add_animation(
        None,
        SlideAnimation::new(1.2, move |scene: &mut Scene, p| {
            place(scene, -1.5 * (1.0 - p), 3.0, p)
        })
        .easing(Easing::BackOut)
        .on_finish(move |scene: &mut Scene| {
            scene.add_animation(None, fade_out);
        })
        .target(node),
    );
    Ok(())
}

/// Colour of hue `hue` (0..1 around the colour wheel), `saturation` and `value` (0..1).
fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> Vec3 {
    let h = (hue.fract() * 6.0).max(0.0);
    let c = value * saturation;
    let x = c * (1.0 - ((h % 2.0) - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = value - c;
    Vec3::new(r + m, g + m, b + m)
}

impl Demo for AnimationDemo {
    fn scene(&mut self) -> &mut Scene {
        &mut self.scene
    }

    fn camera(&mut self) -> &mut dyn Camera {
        &mut self.rig.camera
    }

    fn update(&mut self, time: f32) -> Result<()> {
        self.camera_animator.advance_to(&mut self.rig, time);
        self.rig.apply();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_lane_kind_has_its_own_easings() {
        assert_eq!(easing_for(0, 2), Easing::Linear);
        assert_eq!(easing_for(1, 0), Easing::QuadraticIn);
        assert_eq!(easing_for(1, 2), Easing::QuadraticInOut);
        assert_eq!(easing_for(KINDS - 1, 1), Easing::BounceOut);
    }

    #[test]
    fn colours_stay_in_range() {
        for i in 0..24 {
            let c = hsv_to_rgb(i as f32 / 24.0, 0.7, 0.95);
            for v in [c.x, c.y, c.z] {
                assert!((0.0..=1.0).contains(&v));
            }
        }
    }
}
