use anyhow::Result;

use crate::label::Label;
use crate::rect::Rect;
use crate::widget::{
    quad_transform, EventContext, LayoutContext, UiEvent, Widget, WidgetBase, WidgetId,
};

/// A button: a rectangle with a material per state, and optionally a label in its middle.
///
/// Pressing it takes the press; releasing it over the button reports `UiEvent::Clicked`, and
/// releasing it elsewhere doesn't, so a press can be taken back by moving away. It is drawn
/// with `material`, `hover_material` while the pointer is over it, and `pressed_material`
/// while pressed with the pointer over it; a missing state material falls back to the normal
/// one.
///
/// It has no preferred size of its own: set one (`Widget::with_size`) unless it fills its
/// space.
pub struct Button {
    base: WidgetBase,
    material: Option<usize>,
    hover_material: Option<usize>,
    pressed_material: Option<usize>,
    label: Option<Label>,
    background: Option<usize>,
    hover: bool,
    pressed: bool,
}

impl Default for Button {
    fn default() -> Self {
        Self::new()
    }
}

impl Button {
    pub fn new() -> Self {
        Self {
            base: WidgetBase::new(),
            material: None,
            hover_material: None,
            pressed_material: None,
            label: None,
            background: None,
            hover: false,
            pressed: false,
        }
    }

    /// The material of the background.
    pub fn material(mut self, material: usize) -> Self {
        self.material = Some(material);
        self
    }

    /// The material while the pointer is over the button.
    pub fn hover_material(mut self, material: usize) -> Self {
        self.hover_material = Some(material);
        self
    }

    /// The material while the button is pressed (with the pointer over it).
    pub fn pressed_material(mut self, material: usize) -> Self {
        self.pressed_material = Some(material);
        self
    }

    /// A label centered on the button. Its own size, alignment and margin don't matter.
    pub fn label(mut self, label: Label) -> Self {
        self.label = Some(label);
        self
    }

    pub fn get_label(&self) -> Option<&Label> {
        self.label.as_ref()
    }

    pub fn get_label_mut(&mut self) -> Option<&mut Label> {
        self.label.as_mut()
    }

    pub fn is_hovered(&self) -> bool {
        self.hover
    }

    /// Whether the button is pressed (it may not be clicked: the pointer can be elsewhere).
    pub fn is_pressed(&self) -> bool {
        self.pressed
    }

    /// The node drawing the background, once the button has been laid out.
    pub fn background_node(&self) -> Option<usize> {
        self.background
    }

    fn current_material(&self) -> Option<usize> {
        let state = if self.pressed && self.hover {
            self.pressed_material
        } else if self.hover {
            self.hover_material
        } else {
            None
        };
        state.or(self.material)
    }

    fn show_state(&self, cx: &mut EventContext) -> Result<()> {
        match (self.background, self.current_material()) {
            (Some(node), Some(material)) => cx.set_material(node, material),
            _ => Ok(()),
        }
    }
}

impl Widget for Button {
    fn base(&self) -> &WidgetBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut WidgetBase {
        &mut self.base
    }

    fn measure(&mut self, cx: &mut LayoutContext) -> Result<()> {
        if let Some(label) = &mut self.label {
            label.update_text(cx)?;
        }
        Ok(())
    }

    fn arrange(&mut self, cx: &mut LayoutContext) -> Result<Vec<(WidgetId, Rect)>> {
        let rect = self.base.rect();
        let background = match self.background {
            Some(node) => node,
            None => *self.background.insert(cx.create_quad(self.current_material())?),
        };
        let local = Rect::from_size(0.0, 0.0, rect.width(), rect.height());
        cx.set_transform(background, quad_transform(local, 0.0))?;
        if let Some(label) = &self.label {
            let size = label.text_size();
            let x = (rect.width() - size.width) / 2.0;
            let y = (rect.height() - size.height) / 2.0;
            label.place_text(cx, x, y)?;
        }
        Ok(Vec::new())
    }

    fn mouse_move(&mut self, x: f32, y: f32, cx: &mut EventContext) -> Result<()> {
        self.hover = self.base.rect().contains(x, y);
        self.show_state(cx)
    }

    fn mouse_down(&mut self, _x: f32, _y: f32, cx: &mut EventContext) -> Result<bool> {
        self.pressed = true;
        self.hover = true;
        self.show_state(cx)?;
        Ok(true)
    }

    fn mouse_drag(&mut self, x: f32, y: f32, cx: &mut EventContext) -> Result<()> {
        self.mouse_move(x, y, cx)
    }

    fn mouse_up(&mut self, x: f32, y: f32, cx: &mut EventContext) -> Result<()> {
        self.pressed = false;
        self.hover = self.base.rect().contains(x, y);
        if self.hover {
            let widget = cx.id();
            cx.emit(UiEvent::Clicked { widget });
        }
        self.show_state(cx)
    }
}
