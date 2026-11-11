use anyhow::Result;

use crate::rect::Rect;
use crate::widget::{
    quad_transform, EventContext, LayoutContext, UiEvent, Visibility, Widget, WidgetBase,
    WidgetId,
};

/// Which way a scrollbar goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    /// Top to bottom: the smallest value at the top.
    Vertical,
    /// Left to right: the smallest value at the left.
    Horizontal,
}

/// How far in front of the track the knob is drawn.
const KNOB_Z: f32 = 0.05;

/// A scrollbar: a track with a knob showing which part of some content is in view.
///
/// The value goes from `min` to `max`: the position of the view's start in the content, so
/// `max` is the content's size minus the view's (`page`). The knob is as long as the view is
/// of the whole content (`page / (max - min + page)` of the track), but at least
/// `min_knob_size`. Clicking the track before or after the knob moves by a page; dragging the
/// knob moves it; the wheel over the scrollbar moves by `wheel_step` per line. Changes made
/// with the mouse are reported (`UiEvent::ValueChanged`); changes made by the app are not.
///
/// With `Visibility::Automatic`, the scrollbar is hidden and takes no space when there is
/// nothing to scroll (`max <= min`). The track and the knob each have a material and one for
/// while the pointer is over the scrollbar (or the knob is being dragged).
pub struct Scrollbar {
    base: WidgetBase,
    orientation: Orientation,
    min: f64,
    max: f64,
    value: f64,
    page: f64,
    min_knob_size: f32,
    wheel_step: Option<f64>,
    track_material: Option<usize>,
    track_hover_material: Option<usize>,
    knob_material: Option<usize>,
    knob_hover_material: Option<usize>,
    track: Option<usize>,
    knob: Option<usize>,
    hover: bool,
    /// While the knob is dragged: where the pointer took it, from the knob's start.
    drag: Option<f32>,
}

impl Scrollbar {
    /// A scrollbar 16 units thick, from 0 to 100 with a page of 10, at 0.
    pub fn new(orientation: Orientation) -> Self {
        Self {
            base: WidgetBase::new(),
            orientation,
            min: 0.0,
            max: 100.0,
            value: 0.0,
            page: 10.0,
            min_knob_size: 10.0,
            wheel_step: None,
            track_material: None,
            track_hover_material: None,
            knob_material: None,
            knob_hover_material: None,
            track: None,
            knob: None,
            hover: false,
            drag: None,
        }
    }

    pub fn vertical() -> Self {
        Self::new(Orientation::Vertical)
    }

    pub fn horizontal() -> Self {
        Self::new(Orientation::Horizontal)
    }

    pub fn track_material(mut self, material: usize) -> Self {
        self.track_material = Some(material);
        self
    }

    pub fn track_hover_material(mut self, material: usize) -> Self {
        self.track_hover_material = Some(material);
        self
    }

    pub fn knob_material(mut self, material: usize) -> Self {
        self.knob_material = Some(material);
        self
    }

    pub fn knob_hover_material(mut self, material: usize) -> Self {
        self.knob_hover_material = Some(material);
        self
    }

    pub fn orientation(&self) -> Orientation {
        self.orientation
    }

    pub fn min(&self) -> f64 {
        self.min
    }

    pub fn max(&self) -> f64 {
        self.max
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn page(&self) -> f64 {
        self.page
    }

    /// Sets the range of values (`max` below `min` counts as `min`), keeping the value in it.
    pub fn set_range(&mut self, min: f64, max: f64) {
        self.min = min;
        self.max = max.max(min);
        self.value = self.clamp(self.value);
    }

    /// The size of the view, in the value's units.
    pub fn set_page(&mut self, page: f64) {
        self.page = page.max(0.0);
    }

    /// Moves to `value`, kept in the range. Not reported as an event.
    pub fn set_value(&mut self, value: f64) {
        self.value = self.clamp(value);
    }

    /// The knob's smallest length, in UI units.
    pub fn set_min_knob_size(&mut self, size: f32) {
        self.min_knob_size = size.max(0.0);
    }

    /// How much a line of wheel moves the value (`None`, the default: a tenth of the page).
    pub fn set_wheel_step(&mut self, step: Option<f64>) {
        self.wheel_step = step;
    }

    /// Whether there is anything to scroll.
    pub fn can_scroll(&self) -> bool {
        self.max > self.min
    }

    /// The knob's length on a track `track` long.
    pub fn knob_size(&self, track: f32) -> f32 {
        let total = self.max - self.min + self.page;
        let size = if total > 0.0 {
            track * (self.page / total) as f32
        } else {
            track
        };
        size.clamp(self.min_knob_size.min(track), track.max(0.0))
    }

    /// Where the knob starts on a track `track` long.
    pub fn knob_position(&self, track: f32) -> f32 {
        if !self.can_scroll() {
            return 0.0;
        }
        let free = (track - self.knob_size(track)).max(0.0);
        ((self.value - self.min) / (self.max - self.min)) as f32 * free
    }

    /// The knob's rectangle, relative to the scrollbar's top left corner.
    pub fn knob_rect(&self) -> Rect {
        let rect = self.base.rect();
        match self.orientation {
            Orientation::Vertical => {
                let size = self.knob_size(rect.height());
                let top = self.knob_position(rect.height());
                Rect::from_size(0.0, top, rect.width(), size)
            }
            Orientation::Horizontal => {
                let size = self.knob_size(rect.width());
                let left = self.knob_position(rect.width());
                Rect::from_size(left, 0.0, size, rect.height())
            }
        }
    }

    fn clamp(&self, value: f64) -> f64 {
        value.clamp(self.min, self.max)
    }

    /// The track's start and length along the scrollbar, and `(x, y)` along it.
    fn along(&self, x: f32, y: f32) -> (f32, f32, f32) {
        let rect = self.base.rect();
        match self.orientation {
            Orientation::Vertical => (rect.top, rect.height(), y),
            Orientation::Horizontal => (rect.left, rect.width(), x),
        }
    }

    /// Moves to `value` because of the mouse: reports it if it changed.
    fn move_to(&mut self, value: f64, cx: &mut EventContext) {
        let value = self.clamp(value);
        if value != self.value {
            self.value = value;
            let widget = cx.id();
            cx.emit(UiEvent::ValueChanged { widget, value });
            cx.request_layout();
        }
    }

    fn show_state(&self, cx: &mut EventContext) -> Result<()> {
        let lit = self.hover || self.drag.is_some();
        let pick = |normal: Option<usize>, hover: Option<usize>| {
            if lit {
                hover.or(normal)
            } else {
                normal
            }
        };
        let parts = [
            (self.track, pick(self.track_material, self.track_hover_material)),
            (self.knob, pick(self.knob_material, self.knob_hover_material)),
        ];
        for (node, material) in parts {
            if let (Some(node), Some(material)) = (node, material) {
                cx.set_material(node, material)?;
            }
        }
        Ok(())
    }
}

impl Widget for Scrollbar {
    fn base(&self) -> &WidgetBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut WidgetBase {
        &mut self.base
    }

    fn takes_space(&self) -> bool {
        match self.base.visibility() {
            Visibility::Automatic => self.can_scroll(),
            _ => self.base.default_takes_space(),
        }
    }

    fn is_visible(&self) -> bool {
        match self.base.visibility() {
            Visibility::Automatic => self.can_scroll(),
            _ => self.base.default_is_visible(),
        }
    }

    fn arrange(&mut self, cx: &mut LayoutContext) -> Result<Vec<(WidgetId, Rect)>> {
        let track = match self.track {
            Some(node) => node,
            None => *self.track.insert(cx.create_quad(self.track_material)?),
        };
        let knob = match self.knob {
            Some(node) => node,
            None => *self.knob.insert(cx.create_quad(self.knob_material)?),
        };
        let rect = self.base.rect();
        let local = Rect::from_size(0.0, 0.0, rect.width(), rect.height());
        cx.set_transform(track, quad_transform(local, 0.0))?;
        cx.set_transform(knob, quad_transform(self.knob_rect(), KNOB_Z))?;
        Ok(Vec::new())
    }

    fn mouse_move(&mut self, x: f32, y: f32, cx: &mut EventContext) -> Result<()> {
        self.hover = self.base.rect().contains(x, y);
        self.show_state(cx)
    }

    fn mouse_down(&mut self, x: f32, y: f32, cx: &mut EventContext) -> Result<bool> {
        let (start, track, at) = self.along(x, y);
        let knob_start = start + self.knob_position(track);
        let knob_end = knob_start + self.knob_size(track);
        if at < knob_start {
            self.move_to(self.value - self.page, cx);
        } else if at > knob_end {
            self.move_to(self.value + self.page, cx);
        } else {
            self.drag = Some(at - knob_start);
        }
        self.hover = true;
        self.show_state(cx)?;
        Ok(true)
    }

    fn mouse_drag(&mut self, x: f32, y: f32, cx: &mut EventContext) -> Result<()> {
        self.hover = self.base.rect().contains(x, y);
        if let Some(offset) = self.drag {
            let (start, track, at) = self.along(x, y);
            let free = track - self.knob_size(track);
            if free > 0.0 {
                let fraction = ((at - start - offset) / free) as f64;
                self.move_to(self.min + fraction * (self.max - self.min), cx);
            }
        }
        self.show_state(cx)
    }

    fn mouse_up(&mut self, x: f32, y: f32, cx: &mut EventContext) -> Result<()> {
        self.mouse_drag(x, y, cx)?;
        self.drag = None;
        self.show_state(cx)
    }

    fn mouse_wheel(
        &mut self,
        delta_x: f32,
        delta_y: f32,
        _x: f32,
        _y: f32,
        cx: &mut EventContext,
    ) -> Result<bool> {
        if !self.can_scroll() {
            return Ok(false);
        }
        let step = self.wheel_step.unwrap_or(self.page / 10.0);
        // Turning the wheel away from the user scrolls up (or left): towards `min`.
        let lines = match self.orientation {
            Orientation::Horizontal if delta_x != 0.0 => delta_x,
            _ => -delta_y,
        };
        self.move_to(self.value + lines as f64 * step, cx);
        Ok(true)
    }
}
