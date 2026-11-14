use anyhow::{anyhow, Result};

use feather::assetsprovider::AssetsProvider;
use feather::featherapp::FrameStats;
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
/// The widest frame rate text the panel leaves room for.
const WIDEST_FPS: &str = "8888 fps";
/// The widest frame stats text the panel leaves room for.
const WIDEST_STATS: &str = "88888 draws  88888 culled  88888 shadow  88.88 ms";
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
    /// The engine's latest frame stats, and the collect time summed over the frames since
    /// they were last shown.
    stats: FrameStats,
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
        let stats_width = stats_builder.measure(&scene, WIDEST_STATS)?.width;
        let stats = Label::new(stats_builder, stats_material, "");
        let panel = InfoPanel::new(title, fps, stats, fps_width.max(stats_width), panel_material)
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
            stats: FrameStats::default(),
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
    /// The frame stats text shown.
    pub fn get_frame_stats_text(&self) -> &str {
        self.hud.get(self.panel).unwrap().stats.text()
    }

    /// The engine's stats of the frame just collected; shown with the frame rate, the collect
    /// time averaged over the frames since the last time.
    pub fn set_frame_stats(&mut self, stats: &FrameStats) {
        self.stats = *stats;
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
                let shown = format!("{:.0} fps", self.frames as f32 / self.seconds);
                self.panel_mut().fps.set_text(shown);
                if self.stats_frames > 0 {
                    let s = self.stats;
                    let ms = self.collect_time / self.stats_frames as f32 * 1000.0;
                    let shown = format!(
                        "{} draws  {} culled  {} shadow  {ms:.2} ms",
                        s.draws, s.culled, s.shadow_draws
                    );
                    self.panel_mut().stats.set_text(shown);
                    self.collect_time = 0.0;
                    self.stats_frames = 0;
                }
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

/// The panel in the bottom left corner: a see-through background around three lines, the
/// title, the frame rate and the frame stats. It is as wide as the title or the widest frame
/// rate and stats (so it doesn't change size with them), and ignores the mouse.
pub struct InfoPanel {
    base: WidgetBase,
    title: Label,
    fps: Label,
    stats: Label,
    /// The width of the widest frame rate or stats line.
    fps_width: f32,
    material: usize,
    background: Option<usize>,
}

impl InfoPanel {
    fn new(title: Label, fps: Label, stats: Label, fps_width: f32, material: usize) -> Self {
        Self {
            base: WidgetBase::new(),
            title,
            fps,
            stats,
            fps_width,
            material,
            background: None,
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
        self.stats.update_text(cx)?;
        let width = title.width.max(self.fps_width) + 2.0 * PADDING;
        let height =
            TITLE_HEIGHT + LINE_GAP + FPS_HEIGHT + LINE_GAP + STATS_HEIGHT + 2.0 * PADDING;
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
        self.stats
            .place_text(cx, PADDING, fps_top + FPS_HEIGHT + LINE_GAP)?;
        Ok(Vec::new())
    }
}
