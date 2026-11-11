//! What every widget has: the `Widget` trait, its common state (`WidgetBase`), handles
//! (`WidgetId`) and what the hud gives widgets while laying them out or handling the mouse
//! (`LayoutContext`, `EventContext`).

use std::any::Any;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use anyhow::{anyhow, Result};
use feather::math::{Mat4, Vec3};
use feather::scene::Scene;

use crate::hud::Store;
use crate::rect::Rect;

/// Whether a widget is drawn and takes space in its layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Visibility {
    /// The widget decides; most widgets are visible. A scrollbar hides itself when there is
    /// nothing to scroll.
    #[default]
    Automatic,
    /// Not drawn, and takes no space: the layout gives its space to the others.
    Gone,
    /// Not drawn, but takes its space.
    Invisible,
    /// Drawn.
    Visible,
}

/// Where a widget goes in the space its layout gives it, along one axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    /// The widget fills the space (its preferred size is ignored on this axis).
    #[default]
    Fill,
    /// The widget keeps its preferred size, at the left (or top) of the space.
    Start,
    /// The widget keeps its preferred size, in the middle of the space.
    Center,
    /// The widget keeps its preferred size, at the right (or bottom) of the space.
    End,
}

impl Align {
    /// The part of `start..end` taken by something of `size`.
    fn place(self, start: f32, end: f32, size: f32) -> (f32, f32) {
        let size = size.min(end - start).max(0.0);
        match self {
            Align::Fill => (start, end),
            Align::Start => (start, start + size),
            Align::Center => {
                let left = start + (end - start - size) / 2.0;
                (left, left + size)
            }
            Align::End => (end - size, end),
        }
    }
}

/// What every widget has: the rectangle it was laid out in, its preferred size, its
/// visibility, and how it is placed in the space its layout gives it (alignment and margin).
#[derive(Clone, Debug)]
pub struct WidgetBase {
    rect: Rect,
    width: f32,
    height: f32,
    visibility: Visibility,
    align_x: Align,
    align_y: Align,
    margin: f32,
}

impl Default for WidgetBase {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetBase {
    /// A visible widget 16 × 16 units in size, filling its space, with no margin.
    pub fn new() -> Self {
        Self {
            rect: Rect::from_size(0.0, 0.0, 16.0, 16.0),
            width: 16.0,
            height: 16.0,
            visibility: Visibility::Automatic,
            align_x: Align::Fill,
            align_y: Align::Fill,
            margin: 0.0,
        }
    }

    /// The rectangle the widget was last laid out in, in UI units.
    pub fn rect(&self) -> Rect {
        self.rect
    }

    pub(crate) fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    /// The preferred width: what a layout gives a widget on its side (`LayoutNswe`'s west and
    /// east), and what the widget keeps when not aligned with `Fill`.
    pub fn width(&self) -> f32 {
        self.width
    }

    pub fn height(&self) -> f32 {
        self.height
    }

    pub fn set_size(&mut self, width: f32, height: f32) {
        self.width = width;
        self.height = height;
    }

    /// The preferred width with the margins: the space the widget asks its layout for.
    pub fn outer_width(&self) -> f32 {
        self.width + 2.0 * self.margin
    }

    pub fn outer_height(&self) -> f32 {
        self.height + 2.0 * self.margin
    }

    pub fn visibility(&self) -> Visibility {
        self.visibility
    }

    pub fn set_visibility(&mut self, visibility: Visibility) {
        self.visibility = visibility;
    }

    pub fn align(&self) -> (Align, Align) {
        (self.align_x, self.align_y)
    }

    /// How the widget is placed horizontally and vertically in its space.
    pub fn set_align(&mut self, horizontal: Align, vertical: Align) {
        self.align_x = horizontal;
        self.align_y = vertical;
    }

    pub fn margin(&self) -> f32 {
        self.margin
    }

    /// Space kept free around the widget, on every side.
    pub fn set_margin(&mut self, margin: f32) {
        self.margin = margin;
    }

    /// The rectangle the widget takes in `space`: the space without the margin, then the
    /// preferred size placed by the alignment.
    pub fn place(&self, space: Rect) -> Rect {
        let inner = space.shrink(self.margin);
        let (left, right) = self.align_x.place(inner.left, inner.right, self.width);
        let (top, bottom) = self.align_y.place(inner.top, inner.bottom, self.height);
        Rect::new(left, top, right, bottom)
    }

    /// Whether the visibility alone makes the widget take space (all but `Gone`).
    pub fn default_takes_space(&self) -> bool {
        self.visibility != Visibility::Gone
    }

    /// Whether the visibility alone makes the widget drawn (`Automatic` and `Visible`).
    pub fn default_is_visible(&self) -> bool {
        matches!(self.visibility, Visibility::Automatic | Visibility::Visible)
    }
}

/// A handle to a widget in a `Hud`. `T` is the widget's type, so `Hud::get` returns it
/// without a cast; `WidgetId` alone (`WidgetId<dyn Widget>`) is any widget, as layouts and
/// events hold them. Handles of any types compare equal when they name the same widget.
pub struct WidgetId<T: ?Sized = dyn Widget> {
    index: usize,
    _type: PhantomData<fn() -> *const T>,
}

impl<T: ?Sized> WidgetId<T> {
    pub(crate) fn new(index: usize) -> Self {
        Self {
            index,
            _type: PhantomData,
        }
    }

    pub(crate) fn index(self) -> usize {
        self.index
    }

    /// The same handle, for any widget type.
    pub fn untyped(self) -> WidgetId {
        WidgetId::new(self.index)
    }
}

impl<T: Widget> From<WidgetId<T>> for WidgetId {
    fn from(id: WidgetId<T>) -> Self {
        id.untyped()
    }
}

impl<T: ?Sized> Clone for WidgetId<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for WidgetId<T> {}

impl<T: ?Sized, U: ?Sized> PartialEq<WidgetId<U>> for WidgetId<T> {
    fn eq(&self, other: &WidgetId<U>) -> bool {
        self.index == other.index
    }
}

impl<T: ?Sized> Eq for WidgetId<T> {}

impl<T: ?Sized> Hash for WidgetId<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}

impl<T: ?Sized> fmt::Debug for WidgetId<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WidgetId({})", self.index)
    }
}

/// Something that happened to a widget because of the mouse; read with `Hud::take_events`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UiEvent {
    /// A button was clicked: pressed and released over it.
    Clicked { widget: WidgetId },
    /// The user changed a scrollbar's value (not reported when the app sets it).
    ValueChanged { widget: WidgetId, value: f64 },
}

/// A widget: a part of the UI with a rectangle, drawn with nodes in the hud's scene, that can
/// take mouse input. Layouts are widgets that place other widgets (`children`).
///
/// The hud lays widgets out in two passes, from its root widget down: `measure` (children
/// before their parent) lets widgets work out their preferred size, then `arrange` (parents
/// before their children) gives each its rectangle (`WidgetBase::rect`) and lets it place its
/// nodes, and layouts say where their children go.
///
/// Positions are in UI units from the hud's top left corner, Y down. Each widget has a node
/// of its own (`LayoutContext::anchor`) at the top left corner of its rectangle, which the hud
/// moves and hides; the widget's own nodes go under it.
///
/// Apps can write their own widgets.
pub trait Widget: Any + Send + Sync {
    fn base(&self) -> &WidgetBase;
    fn base_mut(&mut self) -> &mut WidgetBase;

    /// The widgets this one places, for layouts.
    fn children(&self) -> Vec<WidgetId> {
        Vec::new()
    }

    /// Whether the widget takes space in its layout. The default follows the visibility.
    fn takes_space(&self) -> bool {
        self.base().default_takes_space()
    }

    /// Whether the widget is drawn. The default follows the visibility.
    fn is_visible(&self) -> bool {
        self.base().default_is_visible()
    }

    /// Works out the preferred size (`WidgetBase::set_size`), e.g. from a text. Children are
    /// measured first.
    fn measure(&mut self, _cx: &mut LayoutContext) -> Result<()> {
        Ok(())
    }

    /// Places the widget's nodes in its rectangle (`WidgetBase::rect`, already set), and
    /// returns where its children go.
    fn arrange(&mut self, _cx: &mut LayoutContext) -> Result<Vec<(WidgetId, Rect)>> {
        Ok(Vec::new())
    }

    /// The pointer moved to `(x, y)`, anywhere. Every visible widget hears it, e.g. to light
    /// up while the pointer is over it.
    fn mouse_move(&mut self, _x: f32, _y: f32, _cx: &mut EventContext) -> Result<()> {
        Ok(())
    }

    /// A button was pressed at `(x, y)`, in this widget's rectangle. Returns whether the widget
    /// takes it: then it gets the drag (`mouse_drag`) and the release (`mouse_up`), wherever
    /// the pointer goes.
    fn mouse_down(&mut self, _x: f32, _y: f32, _cx: &mut EventContext) -> Result<bool> {
        Ok(false)
    }

    /// The pointer moved to `(x, y)` while this widget has the press.
    fn mouse_drag(&mut self, _x: f32, _y: f32, _cx: &mut EventContext) -> Result<()> {
        Ok(())
    }

    /// The button was released at `(x, y)`, after this widget took the press.
    fn mouse_up(&mut self, _x: f32, _y: f32, _cx: &mut EventContext) -> Result<()> {
        Ok(())
    }

    /// The wheel turned with the pointer at `(x, y)`, in this widget's rectangle (see
    /// `FeatherApp::on_mouse_wheel` for the deltas). Returns whether the widget took it.
    fn mouse_wheel(
        &mut self,
        _delta_x: f32,
        _delta_y: f32,
        _x: f32,
        _y: f32,
        _cx: &mut EventContext,
    ) -> Result<bool> {
        Ok(false)
    }

    /// Sets the preferred size (`WidgetBase::set_size`), for building widgets.
    fn with_size(mut self, width: f32, height: f32) -> Self
    where
        Self: Sized,
    {
        self.base_mut().set_size(width, height);
        self
    }

    /// Sets the alignment (`WidgetBase::set_align`), for building widgets.
    fn with_align(mut self, horizontal: Align, vertical: Align) -> Self
    where
        Self: Sized,
    {
        self.base_mut().set_align(horizontal, vertical);
        self
    }

    /// Sets the margin (`WidgetBase::set_margin`), for building widgets.
    fn with_margin(mut self, margin: f32) -> Self
    where
        Self: Sized,
    {
        self.base_mut().set_margin(margin);
        self
    }

    /// Sets the visibility (`WidgetBase::set_visibility`), for building widgets.
    fn with_visibility(mut self, visibility: Visibility) -> Self
    where
        Self: Sized,
    {
        self.base_mut().set_visibility(visibility);
        self
    }
}

/// The transform putting the hud's unit square (`0..1` on X and Y) over `rect`, given in UI
/// units relative to a widget's top left corner (Y down), at depth `z`: nodes with a greater
/// `z` are drawn over the others.
pub fn quad_transform(rect: Rect, z: f32) -> Mat4 {
    Mat4::from_translation(Vec3::new(rect.left, -rect.bottom, z))
        * Mat4::from_nonuniform_scale(rect.width(), rect.height(), 1.0)
}

/// The transform putting a node at `(x, y)` relative to a widget's top left corner (Y down),
/// at depth `z`.
pub fn position_transform(x: f32, y: f32, z: f32) -> Mat4 {
    Mat4::from_translation(Vec3::new(x, -y, z))
}

/// What a widget gets while being measured and arranged.
pub struct LayoutContext<'a> {
    pub(crate) scene: &'a mut Scene,
    pub(crate) anchor: usize,
    pub(crate) square: usize,
    pub(crate) store: &'a Store,
}

impl LayoutContext<'_> {
    /// The hud's scene, where the widget's nodes are.
    pub fn scene(&mut self) -> &mut Scene {
        self.scene
    }

    /// The widget's node: at the top left corner of its rectangle, with Y up as everywhere in
    /// the scene (so Y down in the widget is -Y here; `quad_transform` and
    /// `position_transform` convert). The widget's nodes go under it.
    pub fn anchor(&self) -> usize {
        self.anchor
    }

    /// The hud's square mesh, `0..1` on X and Y; see `quad_transform`.
    pub fn square_mesh(&self) -> usize {
        self.square
    }

    /// Another widget (not the one being laid out), e.g. a layout's child.
    pub fn widget(&self, id: WidgetId) -> Option<&dyn Widget> {
        self.store.get(id)
    }

    /// Creates a transparent node under the widget's anchor drawing the square mesh with
    /// `material` (none: not drawn until it gets one).
    pub fn create_quad(&mut self, material: Option<usize>) -> Result<usize> {
        let node = self.scene.create_node(None, self.anchor)?;
        let scene = &mut *self.scene;
        let n = scene.get_node_mut(node).unwrap();
        n.set_transparent(true);
        n.set_mesh(self.square);
        if let Some(material) = material {
            scene.node_set_material(node, material)?;
        }
        Ok(node)
    }

    /// Sets a node's transform (see `quad_transform` and `position_transform`).
    pub fn set_transform(&mut self, node: usize, transform: Mat4) -> Result<()> {
        self.scene
            .get_node_mut(node)
            .ok_or_else(|| anyhow!("node {node} not found"))?
            .set_transform(transform);
        Ok(())
    }
}

/// What a widget gets while handling the mouse.
pub struct EventContext<'a> {
    pub(crate) scene: &'a mut Scene,
    pub(crate) anchor: usize,
    pub(crate) id: WidgetId,
    pub(crate) events: &'a mut Vec<UiEvent>,
    pub(crate) relayout: &'a mut bool,
}

impl EventContext<'_> {
    /// The hud's scene, where the widget's nodes are.
    pub fn scene(&mut self) -> &mut Scene {
        self.scene
    }

    /// The widget's node (see `LayoutContext::anchor`).
    pub fn anchor(&self) -> usize {
        self.anchor
    }

    /// The handle of the widget handling the event.
    pub fn id(&self) -> WidgetId {
        self.id
    }

    /// Reports an event to the app (`Hud::take_events`).
    pub fn emit(&mut self, event: UiEvent) {
        self.events.push(event);
    }

    /// Asks the hud to lay the widgets out again before they are next drawn, e.g. after the
    /// widget's size or contents changed.
    pub fn request_layout(&mut self) {
        *self.relayout = true;
    }

    /// Gives `node` the material `material`, if it has another one.
    pub fn set_material(&mut self, node: usize, material: usize) -> Result<()> {
        let current = self.scene.get_node(node).and_then(|n| n.get_material());
        if current != Some(material) {
            self.scene.node_set_material(node, material)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alignment_places_the_preferred_size_inside_the_margin() {
        let mut base = WidgetBase::new();
        base.set_size(20.0, 10.0);
        base.set_margin(5.0);
        let space = Rect::new(0.0, 0.0, 100.0, 50.0);

        assert_eq!(base.place(space), Rect::new(5.0, 5.0, 95.0, 45.0));
        base.set_align(Align::Start, Align::End);
        assert_eq!(base.place(space), Rect::new(5.0, 35.0, 25.0, 45.0));
        base.set_align(Align::Center, Align::Center);
        assert_eq!(base.place(space), Rect::new(40.0, 20.0, 60.0, 30.0));
        base.set_align(Align::End, Align::Start);
        assert_eq!(base.place(space), Rect::new(75.0, 5.0, 95.0, 15.0));

        // Less space than the preferred size: the widget shrinks to fit.
        assert_eq!(
            base.place(Rect::new(0.0, 0.0, 20.0, 12.0)),
            Rect::new(5.0, 5.0, 15.0, 7.0)
        );
        assert_eq!((base.outer_width(), base.outer_height()), (30.0, 20.0));
    }

    #[test]
    fn visibility_decides_space_and_drawing() {
        let mut base = WidgetBase::new();
        let cases = [
            (Visibility::Automatic, true, true),
            (Visibility::Visible, true, true),
            (Visibility::Invisible, true, false),
            (Visibility::Gone, false, false),
        ];
        for (visibility, space, drawn) in cases {
            base.set_visibility(visibility);
            assert_eq!(base.default_takes_space(), space, "{visibility:?}");
            assert_eq!(base.default_is_visible(), drawn, "{visibility:?}");
        }
    }

    #[test]
    fn quads_cover_their_rectangle_with_y_down() {
        let transform = quad_transform(Rect::from_size(10.0, 20.0, 30.0, 40.0), 0.5);
        let corner = |x: f32, y: f32| {
            let p = transform * feather::math::Vec4::new(x, y, 0.0, 1.0);
            (p.x, p.y, p.z)
        };
        // The square's bottom left corner is the rectangle's bottom left corner, Y flipped.
        assert_eq!(corner(0.0, 0.0), (10.0, -60.0, 0.5));
        assert_eq!(corner(1.0, 1.0), (40.0, -20.0, 0.5));
    }

    #[test]
    fn handles_of_any_type_compare_by_widget() {
        struct A;
        let a: WidgetId<A> = WidgetId::new(3);
        let any: WidgetId = a.untyped();
        assert_eq!(a, any);
        assert_ne!(any, WidgetId::<A>::new(4));
    }
}
