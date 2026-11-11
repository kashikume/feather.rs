//! A small UI library on top of feather, after the C++ engine's `featherui`: widgets
//! (layouts, labels, buttons, scrollbars) laid out in the window, drawn in a scene of their
//! own and fed the mouse by a `Hud`.
//!
//! ```ignore
//! let mut hud = Hud::new(Scene::load(assets, "ui_scene.json")?)?;
//! let next = hud.add(Button::new().material(m).with_size(48.0, 96.0)
//!     .with_align(Align::End, Align::Center))?;
//! let root = hud.add(LayoutNswe::new().with(Slot::East, next))?;
//! hud.set_root(Some(root.into()));
//! // Every frame: hud.update()?; on input: hud.mouse_down(x, y)? ...; then
//! for event in hud.take_events() {
//!     if event == (UiEvent::Clicked { widget: next.into() }) { /* ... */ }
//! }
//! ```

pub mod button;
pub mod hud;
pub mod label;
pub mod layout;
pub mod rect;
pub mod scrollbar;
pub mod widget;

pub use button::Button;
pub use hud::Hud;
pub use label::Label;
pub use layout::{LayoutNswe, Slot, Spacer};
pub use rect::Rect;
pub use scrollbar::{Orientation, Scrollbar};
pub use widget::{
    Align, EventContext, LayoutContext, UiEvent, Visibility, Widget, WidgetBase, WidgetId,
};

#[cfg(test)]
mod tests;
