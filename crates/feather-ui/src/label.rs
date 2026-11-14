use anyhow::Result;
use feather::text::{HorizontalAlign, Text, TextBuilder, TextSize, VerticalAlign};

use crate::rect::Rect;
use crate::widget::{position_transform, LayoutContext, Widget, WidgetBase, WidgetId};

/// How far in front of a widget's background text is drawn.
pub(crate) const TEXT_Z: f32 = 0.1;

/// Text, as big as it is: its preferred size is the size of the text, so with an alignment
/// other than `Fill` the label is exactly the text. The text starts at the top left corner of
/// the label's rectangle.
///
/// It is drawn with a `feather::text::Text` in the hud's scene, built from `builder` (font,
/// line height, method, wrapping; the alignment is always left and top) with `material` (see
/// `feather::text::create_text_material`). The text is (re)built when the hud lays the widgets
/// out, so `set_text` needs no scene. Text that changes often should use
/// `TextMethod::NodePerGlyph`, which changes nodes instead of building a mesh.
pub struct Label {
    base: WidgetBase,
    builder: TextBuilder,
    material: usize,
    text: String,
    shown: Option<Text>,
}

impl Label {
    pub fn new(builder: TextBuilder, material: usize, text: impl Into<String>) -> Self {
        Self {
            base: WidgetBase::new(),
            builder: builder.align(HorizontalAlign::Left, VerticalAlign::Top),
            material,
            text: text.into(),
            shown: None,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// Shows `text` from the next layout (`Hud::update`).
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    /// The node showing the text, once the label has been laid out: give it uniforms (e.g. a
    /// colour) through the hud's scene.
    pub fn text_node(&self) -> Option<usize> {
        self.shown.as_ref().map(|t| t.get_node())
    }

    /// The size of the text shown, in UI units (zero before the first layout).
    pub fn text_size(&self) -> TextSize {
        self.shown
            .as_ref()
            .map(|t| t.get_size())
            .unwrap_or_default()
    }

    /// Builds the text under the widget's anchor the first time, then updates it to `text`.
    /// Returns its size.
    ///
    /// The label's `Widget` methods do this; a widget holding labels of its own (not as
    /// widgets in the hud) calls it from its `measure`, then `place_text` from its `arrange`.
    pub fn update_text(&mut self, cx: &mut LayoutContext) -> Result<TextSize> {
        match &mut self.shown {
            Some(shown) => shown.set_text(cx.scene(), &self.text)?,
            None => {
                let anchor = cx.anchor();
                let shown = Text::create(
                    cx.scene(),
                    anchor,
                    None,
                    self.material,
                    self.builder.clone(),
                    &self.text,
                )?;
                self.shown = Some(shown);
            }
        }
        Ok(self.text_size())
    }

    /// Puts the text's top left corner at `(x, y)` relative to the widget's top left corner
    /// (Y down), a little in front of a background at depth 0.
    pub fn place_text(&self, cx: &mut LayoutContext, x: f32, y: f32) -> Result<()> {
        match self.text_node() {
            Some(node) => cx.set_transform(node, position_transform(x, y, TEXT_Z)),
            None => Ok(()),
        }
    }
}

impl Widget for Label {
    fn base(&self) -> &WidgetBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut WidgetBase {
        &mut self.base
    }

    fn measure(&mut self, cx: &mut LayoutContext) -> Result<()> {
        let size = self.update_text(cx)?;
        self.base.set_size(size.width, size.height);
        Ok(())
    }

    fn arrange(&mut self, cx: &mut LayoutContext) -> Result<Vec<(WidgetId, Rect)>> {
        self.place_text(cx, 0.0, 0.0)?;
        Ok(Vec::new())
    }
}
