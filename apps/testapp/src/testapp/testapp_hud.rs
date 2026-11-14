use anyhow::{anyhow, Result};

use feather::assetsprovider::AssetsProvider;
use feather::featherapp::{FrameStats, SceneStats};
use feather::math::Vec4;
use feather::scene::Scene;
use feather::text::{create_text_material, TextBuilder, TextMethod};
use feather_ui::widget::quad_transform;
use feather_ui::{
    Align, Button, Hud, Label, LayoutContext, LayoutNswe, Rect, Slot, UiEvent, Widget,
    WidgetBase, WidgetId,
};

// The HUD is laid out as if the window were `UI_HEIGHT` pixels high, whatever its real size and
// the display scaling: it scales with the window (`Hud::set_scale`).
const UI_HEIGHT: f32 = 768.0;
/// Distance of the panel and the buttons from the window's edges.
const MARGIN: f32 = 12.0;
/// Distance of the texts from the panel's edges.
const PADDING: f32 = 10.0;
/// Text heights, and the space between the lines.
const TITLE_HEIGHT: f32 = 24.0;
const FPS_HEIGHT: f32 = 20.0;
const STATS_HEIGHT: f32 = 16.0;
const LINE_GAP: f32 = 4.0;
/// How often the frame rate shown is updated, in seconds.
const FPS_PERIOD: f32 = 0.5;
/// The widest frame rate and collect time text the panel leaves room for.
const WIDEST_FPS: &str = "8888 fps  88.88 ms";
/// The widest line of a scene's draws the panel leaves room for.
const WIDEST_DRAWS: &str = "Scene 8: 88888 draws  88888 culled  88888 shadow";
/// The widest line of a scene's resources (nodes, meshes, mesh buffers, atlases) the panel
/// leaves room for; it is indented by `COUNTS_INDENT`.
const WIDEST_COUNTS: &str = "88888 nodes  88888 meshes  888 buffers  888 atlases";
const COUNTS_INDENT: f32 = 16.0;
/// The previous / next demo buttons, and their arrows' height.
const BUTTON_WIDTH: f32 = 48.0;
const BUTTON_HEIGHT: f32 = 96.0;
const ARROW_HEIGHT: f32 = 40.0;
const TITLE_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const FPS_COLOR: [f32; 4] = [0.55, 0.95, 0.6, 1.0];
const STATS_COLOR: [f32; 4] = [0.75, 0.8, 0.85, 1.0];

/// What the HUD's buttons ask the app to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudAction {
    PreviousDemo,
    NextDemo,
}

/// The test app's HUD, drawn over every demo (a `feather_ui::Hud`):
///
/// - in the bottom left corner, on a see-through panel, the demo's number and name and, below,
///   the frame rate and what the engine draws: draw calls, nodes culled (outside the camera's
///   view), shadow draws and the time taken to collect them (`InfoPanel`);
/// - on the left and right edges, halfway up, buttons switching to the previous and next demo.
///
/// Being part of the picture (not the window title), it shows in full screen too. It belongs
/// to the app, not the demos: it stays when demos are switched; `set_title` changes the name
/// shown. Its scene (`testapp_hud_scene.json`) holds the font, shaders and materials.
pub struct TestHud {
    hud: Hud,
    panel: WidgetId<InfoPanel>,
    previous: WidgetId<Button>,
    next: WidgetId<Button>,
    /// Frames counted, and the time they took, since the frame rate was last shown.
    frames: u32,
    seconds: f32,
    /// The time of the last `update`.
    last_time: Option<f32>,
    /// The latest stats of every scene, the HUD's included, how many of them are the demo's
    /// (the rest are the HUD's), and the collect time summed over the frames since they were
    /// last shown.
    stats: Vec<SceneStats>,
    demo_scenes: usize,
    collect_time: f32,
    stats_frames: u32,
}

impl TestHud {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        let mut scene = Scene::load(assets, "testapp_hud_scene.json")?;
        let find = |what: &str, handle: Option<usize>| {
            handle.ok_or_else(|| anyhow!("the HUD has no {what}"))
        };
        let font = find("atlas `Font`", scene.find_atlas("Font"))?;
        let text_shader = find("shader `Text`", scene.find_shader("Text"))?;
        let panel_material = find("material `Panel`", scene.find_material("Panel"))?;
        let button_material = find("material `Button`", scene.find_material("Button"))?;
        let hover = find("material `Button hover`", scene.find_material("Button hover"))?;
        let pressed = find("material `Button pressed`", scene.find_material("Button pressed"))?;
        let mut text_material = |name: &str, color: [f32; 4]| -> Result<usize> {
            let material = create_text_material(&mut scene, Some(name.into()), text_shader, font)?;
            let uniforms = scene.get_material_mut(material).unwrap().uniforms_mut();
            uniforms.set("color", Vec4::from(color));
            Ok(material)
        };
        let title_material = text_material("Title text", TITLE_COLOR)?;
        let fps_material = text_material("Frame rate text", FPS_COLOR)?;
        let stats_material = text_material("Frame stats text", STATS_COLOR)?;

        let title = Label::new(TextBuilder::new(font).height(TITLE_HEIGHT), title_material, "");
        let fps_builder = TextBuilder::new(font)
            .height(FPS_HEIGHT)
            .method(TextMethod::NodePerGlyph);
        let fps_width = fps_builder.measure(&scene, WIDEST_FPS)?.width;
        let fps = Label::new(fps_builder, fps_material, "-- fps");
        let stats_builder = TextBuilder::new(font)
            .height(STATS_HEIGHT)
            .method(TextMethod::NodePerGlyph);
        let draws_width = stats_builder.measure(&scene, WIDEST_DRAWS)?.width;
        let counts_width = stats_builder.measure(&scene, WIDEST_COUNTS)?.width + COUNTS_INDENT;
        let panel = InfoPanel::new(
            title,
            fps,
            (stats_builder, stats_material),
            fps_width.max(draws_width).max(counts_width),
            panel_material,
        )
        .with_align(Align::Start, Align::End)
        .with_margin(MARGIN);

        let arrow = |text: &str| {
            Label::new(TextBuilder::new(font).height(ARROW_HEIGHT), title_material, text)
        };
        let button = |text: &str, side: Align| {
            Button::new()
                .material(button_material)
                .hover_material(hover)
                .pressed_material(pressed)
                .label(arrow(text))
                .with_size(BUTTON_WIDTH, BUTTON_HEIGHT)
                .with_align(side, Align::Center)
                .with_margin(MARGIN)
        };

        let mut hud = Hud::new(scene)?;
        let panel = hud.add(panel)?;
        let previous = hud.add(button("<", Align::Start))?;
        let next = hud.add(button(">", Align::End))?;
        // The panel in the outer layout's south, so it keeps the corner (a layout's south is
        // between its west and east); the buttons in an inner layout above it.
        let sides = LayoutNswe::new()
            .with(Slot::West, previous)
            .with(Slot::East, next);
        let sides = hud.add(sides)?;
        let root = LayoutNswe::new()
            .with(Slot::South, panel)
            .with(Slot::Center, sides);
        let root = hud.add(root)?;
        hud.set_root(Some(root.into()));

        Ok(Self {
            hud,
            panel,
            previous,
            next,
            frames: 0,
            seconds: 0.0,
            last_time: None,
            stats: Vec::new(),
            demo_scenes: 0,
            collect_time: 0.0,
            stats_frames: 0,
        })
    }

    /// Shows `title` (e.g. the demo's number and name) above the frame rate.
    pub fn set_title(&mut self, title: &str) {
        self.panel_mut().title.set_text(title);
    }

    #[cfg(test)]
    pub fn get_title(&self) -> &str {
        self.hud.get(self.panel).unwrap().title.text()
    }

    #[cfg(test)]
    /// The frame rate text shown.
    pub fn get_frame_rate_text(&self) -> &str {
        self.hud.get(self.panel).unwrap().fps.text()
    }

    pub fn scene(&mut self) -> &mut Scene {
        self.hud.scene_mut()
    }

    #[cfg(test)]
    /// The lines of the scenes' stats shown: for each scene, its draws and its resources.
    pub fn get_scene_lines(&self) -> Vec<(&str, &str)> {
        let panel = self.hud.get(self.panel).unwrap();
        panel.scene_lines[..panel.shown_scenes]
            .iter()
            .map(|(draws, counts)| (draws.text(), counts.text()))
            .collect()
    }

    /// The engine's stats of the frame just collected, shown with the frame rate: the collect
    /// time averaged over the frames since the last time, then two lines per scene, the demo's
    /// ("Scene 0", ...) and the last `hud_scenes`, the HUD's own, drawn after them ("HUD").
    pub fn set_frame_stats(&mut self, stats: &FrameStats, hud_scenes: usize) {
        self.stats.clone_from(&stats.scenes);
        self.demo_scenes = stats.scenes.len().saturating_sub(hud_scenes);
        self.collect_time += stats.collect_time;
        self.stats_frames += 1;
    }

    #[cfg(test)]
    pub fn hud(&self) -> &Hud {
        &self.hud
    }

    #[cfg(test)]
    /// The buttons switching to the previous and next demo.
    pub fn buttons(&self) -> (WidgetId<Button>, WidgetId<Button>) {
        (self.previous, self.next)
    }

    #[cfg(test)]
    pub fn panel(&self) -> WidgetId<InfoPanel> {
        self.panel
    }

    /// The size of the window's drawing area, in pixels.
    pub fn set_window_size(&mut self, width: u32, height: u32) {
        self.hud.set_window_size(width, height);
        if height > 0 {
            self.hud.set_scale(height as f32 / UI_HEIGHT);
        }
    }

    /// Called once per frame with the app's time in seconds: counts the frame and, every
    /// `FPS_PERIOD`, shows the frame rate; then lays the HUD out if anything changed.
    pub fn update(&mut self, time: f32) -> Result<()> {
        if let Some(last) = self.last_time {
            self.frames += 1;
            self.seconds += time - last;
            if self.seconds >= FPS_PERIOD {
                let mut shown = format!("{:.0} fps", self.frames as f32 / self.seconds);
                if self.stats_frames > 0 {
                    let ms = self.collect_time / self.stats_frames as f32 * 1000.0;
                    shown += &format!("  {ms:.2} ms");
                    let lines = self
                        .stats
                        .iter()
                        .enumerate()
                        .map(|(index, s)| {
                            let name = if index < self.demo_scenes {
                                format!("Scene {index}")
                            } else {
                                "HUD".to_string()
                            };
                            let draws = format!(
                                "{name}: {} draws  {} culled  {} shadow",
                                s.draws, s.culled, s.shadow_draws
                            );
                            let counts = format!(
                                "{} nodes  {} meshes  {} buffers  {} atlases",
                                s.nodes, s.meshes, s.mesh_buffers, s.atlases
                            );
                            (draws, counts)
                        })
                        .collect();
                    self.panel_mut().set_scene_lines(lines);
                    self.collect_time = 0.0;
                    self.stats_frames = 0;
                }
                self.panel_mut().fps.set_text(shown);
                self.frames = 0;
                self.seconds = 0.0;
            }
        }
        self.last_time = Some(time);
        self.hud.update()
    }

    /// The mouse moved (window pixels). Returns whether the HUD has the press.
    pub fn mouse_move(&mut self, x: f32, y: f32) -> Result<bool> {
        self.hud.mouse_move(x, y)
    }

    pub fn mouse_left(&mut self) -> Result<()> {
        self.hud.mouse_left()
    }

    /// The left button went down (`pressed`) or up. Returns whether the HUD took it.
    pub fn mouse_button(&mut self, pressed: bool, x: f32, y: f32) -> Result<bool> {
        if pressed {
            self.hud.mouse_down(x, y)
        } else {
            self.hud.mouse_up(x, y)
        }
    }

    /// The wheel turned. Returns whether the HUD took it.
    pub fn mouse_wheel(&mut self, delta_x: f32, delta_y: f32, x: f32, y: f32) -> Result<bool> {
        self.hud.mouse_wheel(delta_x, delta_y, x, y)
    }

    /// What the buttons clicked since the last call ask for.
    pub fn take_actions(&mut self) -> Vec<HudAction> {
        let (previous, next) = (self.previous, self.next);
        self.hud
            .take_events()
            .into_iter()
            .filter_map(|event| match event {
                UiEvent::Clicked { widget } if widget == previous => {
                    Some(HudAction::PreviousDemo)
                }
                UiEvent::Clicked { widget } if widget == next => Some(HudAction::NextDemo),
                _ => None,
            })
            .collect()
    }

    fn panel_mut(&mut self) -> &mut InfoPanel {
        self.hud.get_mut(self.panel).unwrap()
    }
}

/// The panel in the bottom left corner: a see-through background around the title, the frame
/// rate and collect time, and two lines per scene drawn (its draws, then its resources,
/// indented). It is as wide as the title or the widest frame rate and scene lines (so it doesn't
/// change size with them), as high as its lines, and ignores the mouse.
pub struct InfoPanel {
    base: WidgetBase,
    title: Label,
    fps: Label,
    /// The text builder and material of the scene lines.
    stats_style: (TextBuilder, usize),
    /// A pair of lines per scene: its draws, and its resources. Pairs are added as needed and
    /// kept; those past `shown_scenes` are empty.
    scene_lines: Vec<(Label, Label)>,
    shown_scenes: usize,
    /// The width of the widest frame rate or scene line.
    fps_width: f32,
    material: usize,
    background: Option<usize>,
}

impl InfoPanel {
    fn new(
        title: Label,
        fps: Label,
        stats_style: (TextBuilder, usize),
        fps_width: f32,
        material: usize,
    ) -> Self {
        Self {
            base: WidgetBase::new(),
            title,
            fps,
            stats_style,
            scene_lines: Vec::new(),
            shown_scenes: 0,
            fps_width,
            material,
            background: None,
        }
    }

    /// Shows a pair of lines (draws, resources) per scene, from the next layout.
    fn set_scene_lines(&mut self, lines: Vec<(String, String)>) {
        let (builder, material) = &self.stats_style;
        while self.scene_lines.len() < lines.len() {
            let label = || Label::new(builder.clone(), *material, "");
            self.scene_lines.push((label(), label()));
        }
        self.shown_scenes = lines.len();
        let mut lines = lines.into_iter();
        for (draws, counts) in &mut self.scene_lines {
            let (draws_text, counts_text) = lines.next().unwrap_or_default();
            draws.set_text(draws_text);
            counts.set_text(counts_text);
        }
    }
}

impl Widget for InfoPanel {
    fn base(&self) -> &WidgetBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut WidgetBase {
        &mut self.base
    }

    fn measure(&mut self, cx: &mut LayoutContext) -> Result<()> {
        let title = self.title.update_text(cx)?;
        self.fps.update_text(cx)?;
        for (draws, counts) in &mut self.scene_lines {
            draws.update_text(cx)?;
            counts.update_text(cx)?;
        }
        let width = title.width.max(self.fps_width) + 2.0 * PADDING;
        let scene_lines = 2 * self.shown_scenes;
        let height = TITLE_HEIGHT
            + LINE_GAP
            + FPS_HEIGHT
            + scene_lines as f32 * (LINE_GAP + STATS_HEIGHT)
            + 2.0 * PADDING;
        self.base.set_size(width, height);
        Ok(())
    }

    fn arrange(&mut self, cx: &mut LayoutContext) -> Result<Vec<(WidgetId, Rect)>> {
        let background = match self.background {
            Some(node) => node,
            None => *self.background.insert(cx.create_quad(Some(self.material))?),
        };
        let rect = self.base.rect();
        let local = Rect::from_size(0.0, 0.0, rect.width(), rect.height());
        cx.set_transform(background, quad_transform(local, 0.0))?;
        self.title.place_text(cx, PADDING, PADDING)?;
        let fps_top = PADDING + TITLE_HEIGHT + LINE_GAP;
        self.fps.place_text(cx, PADDING, fps_top)?;
        let mut top = fps_top + FPS_HEIGHT + LINE_GAP;
        for (draws, counts) in &self.scene_lines[..self.shown_scenes] {
            draws.place_text(cx, PADDING, top)?;
            top += STATS_HEIGHT + LINE_GAP;
            counts.place_text(cx, PADDING + COUNTS_INDENT, top)?;
            top += STATS_HEIGHT + LINE_GAP;
        }
        Ok(Vec::new())
    }
}
