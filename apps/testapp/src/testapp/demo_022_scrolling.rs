use anyhow::{anyhow, Result};
use winit::event::MouseButton;

use feather::assetsprovider::AssetsProvider;
use feather::math::Vec4;
use feather::scene::Scene;
use feather::text::{create_text_material, Text, TextBuilder, TextMethod, TextSize};
use feather_ui::widget::{position_transform, quad_transform};
use feather_ui::{
    Align, Hud, Label, LayoutContext, LayoutNswe, Rect, Scrollbar, Slot, Spacer, Widget,
    WidgetBase, WidgetId,
};

use super::testapp::Demo;

/// The UI is laid out as if the window were this many pixels high, like the app's HUD.
const UI_HEIGHT: f32 = 768.0;
/// Room left free for the app's HUD: its buttons on the sides, its panel at the bottom.
const SIDE_ROOM: f32 = 84.0;
const BOTTOM_ROOM: f32 = 104.0;
const TITLE_HEIGHT: f32 = 32.0;
const MARGIN: f32 = 16.0;
const LINE_HEIGHT: f32 = 22.0;
/// Space between the list's edges and its text.
const PADDING: f32 = 10.0;
const SCROLLBAR_WIDTH: f32 = 16.0;
/// How far a line of mouse wheel scrolls the list.
const WHEEL_LINES: f64 = 3.0;
const LINE_COUNT: usize = 80;

const PHRASES: &[&str] = &[
    "The quick brown fox jumps over the lazy dog.",
    "Scrollbars show which part of the list is in view.",
    "Drag the knob, click the track, or turn the wheel.",
    "Lines and letters outside the view are hidden.",
    "Pack my box with five dozen liquor jugs.",
    "Sphinx of black quartz, judge my vow.",
];

/// A long list of text lines in a view with a vertical and a horizontal scrollbar, made with
/// `feather_ui` in a scene of its own (`demo_022_scrolling_scene.json`).
///
/// Drag a knob, click a track to move by a page, or turn the wheel over the list or a
/// scrollbar; resize the window and the scrollbars follow (they hide when everything fits).
/// The app's HUD is drawn over the demo and takes the mouse first, so the demo leaves room for
/// it (`Spacer`s) and gets the rest.
///
/// There is no clipping: the list (`ListView`) shows only whole lines in its view, and hides
/// the letters of a line scrolled sideways past its edges.
pub struct ScrollingDemo {
    hud: Hud,
    list: WidgetId<ListView>,
    vertical: WidgetId<Scrollbar>,
    horizontal: WidgetId<Scrollbar>,
}

impl ScrollingDemo {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        let mut scene = Scene::load(assets, "demo_022_scrolling_scene.json")?;
        let find = |what: &str, handle: Option<usize>| {
            handle.ok_or_else(|| anyhow!("the scene has no {what}"))
        };
        let font = find("atlas `Font`", scene.find_atlas("Font"))?;
        let text_shader = find("shader `Text`", scene.find_shader("Text"))?;
        let material = |name: &str| find(&format!("material `{name}`"), scene.find_material(name));
        let background = material("List")?;
        let (track, track_hover) = (material("Track")?, material("Track hover")?);
        let (knob, knob_hover) = (material("Knob")?, material("Knob hover")?);
        let mut text_material = |name: &str, color: [f32; 4]| -> Result<usize> {
            let material = create_text_material(&mut scene, Some(name.into()), text_shader, font)?;
            let uniforms = scene.get_material_mut(material).unwrap().uniforms_mut();
            uniforms.set("color", Vec4::from(color));
            Ok(material)
        };
        let title_material = text_material("Title text", [1.0, 1.0, 1.0, 1.0])?;
        let line_material = text_material("Line text", [0.82, 0.86, 0.95, 1.0])?;

        let title = Label::new(
            TextBuilder::new(font).height(TITLE_HEIGHT),
            title_material,
            "Scrolling list",
        )
        .with_align(Align::Start, Align::Start)
        .with_margin(MARGIN);
        let lines_builder = TextBuilder::new(font)
            .height(LINE_HEIGHT)
            .method(TextMethod::NodePerGlyph);
        let list = ListView::new(list_lines(), lines_builder, line_material, background);
        let scrollbar = |bar: Scrollbar| {
            let mut bar = bar
                .track_material(track)
                .track_hover_material(track_hover)
                .knob_material(knob)
                .knob_hover_material(knob_hover)
                .with_size(SCROLLBAR_WIDTH, SCROLLBAR_WIDTH);
            // Nothing to scroll until the list is laid out.
            bar.set_range(0.0, 0.0);
            bar.set_wheel_step(Some(WHEEL_LINES * LINE_HEIGHT as f64));
            bar
        };

        let mut hud = Hud::new(scene)?;
        let title = hud.add(title)?;
        let list = hud.add(list)?;
        let vertical = hud.add(scrollbar(Scrollbar::vertical()))?;
        let horizontal = hud.add(scrollbar(Scrollbar::horizontal()))?;
        let view = LayoutNswe::new()
            .with(Slot::Center, list)
            .with(Slot::East, vertical)
            .with(Slot::South, horizontal)
            .with_margin(MARGIN);
        let view = hud.add(view)?;
        let room = |width, height| Spacer::new().with_size(width, height);
        let west = hud.add(room(SIDE_ROOM, 0.0))?;
        let east = hud.add(room(SIDE_ROOM, 0.0))?;
        let south = hud.add(room(0.0, BOTTOM_ROOM))?;
        let root = LayoutNswe::new()
            .with(Slot::North, title)
            .with(Slot::West, west)
            .with(Slot::East, east)
            .with(Slot::South, south)
            .with(Slot::Center, view);
        let root = hud.add(root)?;
        hud.set_root(Some(root.into()));
        hud.set_window_size(1024, 768);

        let mut demo = Self {
            hud,
            list,
            vertical,
            horizontal,
        };
        demo.sync()?;
        Ok(demo)
    }

    /// Fits the scrollbars to the list's view and content, and scrolls the list to their
    /// values. A scrollbar appearing or hiding changes the view, so this repeats until nothing
    /// changes (two or three rounds).
    fn sync(&mut self) -> Result<()> {
        for _ in 0..4 {
            self.hud.update()?;
            if !self.sync_once() {
                break;
            }
        }
        // Values are read every frame; the events aren't needed.
        self.hud.take_events();
        Ok(())
    }

    /// One round of `sync`. Returns whether anything changed.
    fn sync_once(&mut self) -> bool {
        let list = self.hud.get(self.list).unwrap();
        let view = list.base().rect();
        let content = list.content_size();
        let mut changed = false;
        for (bar, content, view) in [
            (self.vertical, content.height, view.height()),
            (self.horizontal, content.width, view.width()),
        ] {
            let max = (content - view).max(0.0) as f64;
            let current = self.hud.get(bar).unwrap();
            if current.max() != max || current.page() != view as f64 {
                let bar = self.hud.get_mut(bar).unwrap();
                bar.set_range(0.0, max);
                bar.set_page(view as f64);
                changed = true;
            }
        }
        let scroll = (
            self.hud.get(self.horizontal).unwrap().value() as f32,
            self.hud.get(self.vertical).unwrap().value() as f32,
        );
        if self.hud.get(self.list).unwrap().scroll() != scroll {
            self.hud.get_mut(self.list).unwrap().set_scroll(scroll);
            changed = true;
        }
        changed
    }

    #[cfg(test)]
    pub fn hud(&mut self) -> &mut Hud {
        &mut self.hud
    }

    #[cfg(test)]
    pub fn parts(&self) -> (WidgetId<ListView>, WidgetId<Scrollbar>, WidgetId<Scrollbar>) {
        (self.list, self.vertical, self.horizontal)
    }
}

/// The demo's lines: numbered, cycling through `PHRASES`, with one much longer line to scroll
/// sideways.
fn list_lines() -> Vec<String> {
    (0..LINE_COUNT)
        .map(|i| {
            let phrase = PHRASES[i % PHRASES.len()];
            if i == 7 {
                format!("{:>3}  {}", i + 1, PHRASES.join(" "))
            } else {
                format!("{:>3}  {phrase}", i + 1)
            }
        })
        .collect()
}

impl Demo for ScrollingDemo {
    fn scene(&mut self) -> &mut Scene {
        self.hud.scene_mut()
    }

    fn update(&mut self, _time: f32) -> Result<()> {
        self.sync()
    }

    fn on_resized(&mut self, width: u32, height: u32) -> Result<()> {
        self.hud.set_window_size(width, height);
        if height > 0 {
            self.hud.set_scale(height as f32 / UI_HEIGHT);
        }
        self.sync()
    }

    fn on_mouse_moved(&mut self, x: f32, y: f32) -> Result<()> {
        self.hud.mouse_move(x, y)?;
        Ok(())
    }

    fn on_mouse_left(&mut self) -> Result<()> {
        self.hud.mouse_left()
    }

    fn on_mouse_button(&mut self, button: MouseButton, pressed: bool, x: f32, y: f32) -> Result<()> {
        if button == MouseButton::Left {
            if pressed {
                self.hud.mouse_down(x, y)?;
            } else {
                self.hud.mouse_up(x, y)?;
            }
        }
        Ok(())
    }

    /// Over a scrollbar, the scrollbar scrolls; anywhere else, the list scrolls up and down.
    fn on_mouse_wheel(&mut self, delta_x: f32, delta_y: f32, x: f32, y: f32) -> Result<()> {
        if !self.hud.mouse_wheel(delta_x, delta_y, x, y)? {
            let bar = self.hud.get_mut(self.vertical).unwrap();
            let step = WHEEL_LINES * LINE_HEIGHT as f64;
            bar.set_value(bar.value() - delta_y as f64 * step);
        }
        Ok(())
    }
}

/// A list of text lines on a background, scrolled by `scroll` (how far its content is moved
/// left and up). Only whole lines inside the view are shown, and of those only the letters
/// inside it: there is no clipping, so this is how the list stays in its rectangle.
pub struct ListView {
    base: WidgetBase,
    lines: Vec<String>,
    builder: TextBuilder,
    material: usize,
    background_material: usize,
    texts: Vec<Text>,
    background: Option<usize>,
    content: TextSize,
    scroll: (f32, f32),
}

impl ListView {
    fn new(
        lines: Vec<String>,
        builder: TextBuilder,
        material: usize,
        background_material: usize,
    ) -> Self {
        Self {
            base: WidgetBase::new(),
            lines,
            builder,
            material,
            background_material,
            texts: Vec::new(),
            background: None,
            content: TextSize::default(),
            scroll: (0.0, 0.0),
        }
    }

    /// The size of all the lines with the padding around them.
    pub fn content_size(&self) -> TextSize {
        self.content
    }

    pub fn scroll(&self) -> (f32, f32) {
        self.scroll
    }

    pub fn set_scroll(&mut self, scroll: (f32, f32)) {
        self.scroll = scroll;
    }

    /// The text of each line.
    #[cfg(test)]
    pub fn texts(&self) -> &[Text] {
        &self.texts
    }
}

impl Widget for ListView {
    fn base(&self) -> &WidgetBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut WidgetBase {
        &mut self.base
    }

    /// Builds the lines' texts the first time.
    fn measure(&mut self, cx: &mut LayoutContext) -> Result<()> {
        if self.texts.len() == self.lines.len() {
            return Ok(());
        }
        let anchor = cx.anchor();
        let mut width: f32 = 0.0;
        for line in &self.lines {
            let text = Text::create(
                cx.scene(),
                anchor,
                None,
                self.material,
                self.builder.clone(),
                line,
            )?;
            width = width.max(text.get_size().width);
            self.texts.push(text);
        }
        self.content = TextSize {
            width: width + 2.0 * PADDING,
            height: self.lines.len() as f32 * LINE_HEIGHT + 2.0 * PADDING,
        };
        Ok(())
    }

    fn arrange(&mut self, cx: &mut LayoutContext) -> Result<Vec<(WidgetId, Rect)>> {
        let background = match self.background {
            Some(node) => node,
            None => *self
                .background
                .insert(cx.create_quad(Some(self.background_material))?),
        };
        let rect = self.base.rect();
        let (width, height) = (rect.width(), rect.height());
        cx.set_transform(
            background,
            quad_transform(Rect::from_size(0.0, 0.0, width, height), 0.0),
        )?;

        let x = PADDING - self.scroll.0;
        for (i, text) in self.texts.iter().enumerate() {
            let y = PADDING + i as f32 * LINE_HEIGHT - self.scroll.1;
            let shown = y >= 0.0 && y + LINE_HEIGHT <= height;
            let scene = cx.scene();
            let node = scene.get_node_mut(text.get_node()).unwrap();
            node.set_visible(shown);
            node.set_transform(position_transform(x, y, 0.1));
            if !shown {
                continue;
            }
            for &glyph in text.get_glyph_nodes() {
                let glyph = scene.get_node_mut(glyph).unwrap();
                let transform = glyph.get_transform();
                // The glyph's left edge and width, along the line.
                let left = x + transform.w.x;
                let right = left + transform.x.x;
                glyph.set_visible(left >= 0.0 && right <= width);
            }
        }
        Ok(Vec::new())
    }
}
