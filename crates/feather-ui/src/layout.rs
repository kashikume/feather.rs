use anyhow::Result;

use crate::rect::Rect;
use crate::widget::{LayoutContext, Widget, WidgetBase, WidgetId};

/// A slot of `LayoutNswe`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    North,
    South,
    East,
    West,
    Center,
}

const SLOTS: [Slot; 5] = [Slot::North, Slot::South, Slot::East, Slot::West, Slot::Center];

/// A layout with five slots: west and east get their widget's width (with its margins) and
/// north and south its height; center gets what is left.
///
/// ```text
///         | north  |
///    -----+--------+-----
///    west | center | east
///    -----+--------+-----
///         | south  |
/// ```
///
/// West and east span between north and south, and north and south between west and east,
/// so the corners stay empty (as in C++ featherui's `layout_nswe`). To use a corner, nest a
/// layout in the center of another. Slot widgets that take no space (`Visibility::Gone`) give
/// it to the center.
#[derive(Default)]
pub struct LayoutNswe {
    base: WidgetBase,
    slots: [Option<WidgetId>; 5],
}

impl LayoutNswe {
    pub fn new() -> Self {
        Self::default()
    }

    /// Puts `widget` in `slot` (`None` empties it).
    pub fn set(&mut self, slot: Slot, widget: Option<WidgetId>) {
        self.slots[slot as usize] = widget;
    }

    /// `set`, for building layouts.
    pub fn with(mut self, slot: Slot, widget: impl Into<WidgetId>) -> Self {
        self.set(slot, Some(widget.into()));
        self
    }

    pub fn get(&self, slot: Slot) -> Option<WidgetId> {
        self.slots[slot as usize]
    }

    /// The space of each filled slot within `rect`. `size` gives a slot widget's outer width and
    /// height if it takes space.
    pub fn slot_rects(
        &self,
        rect: Rect,
        size: impl Fn(WidgetId) -> Option<(f32, f32)>,
    ) -> Vec<(WidgetId, Rect)> {
        let taken = |slot: Slot| self.get(slot).and_then(&size);
        let mut center = rect;
        if let Some((width, _)) = taken(Slot::West) {
            center.left += width;
        }
        if let Some((width, _)) = taken(Slot::East) {
            center.right -= width;
        }
        if let Some((_, height)) = taken(Slot::North) {
            center.top += height;
        }
        if let Some((_, height)) = taken(Slot::South) {
            center.bottom -= height;
        }
        // Not enough room: the sides overlap rather than the center turning inside out.
        center.right = center.right.max(center.left);
        center.bottom = center.bottom.max(center.top);

        SLOTS
            .iter()
            .filter_map(|&slot| {
                let widget = self.get(slot)?;
                let space = match slot {
                    Slot::North => Rect::new(center.left, rect.top, center.right, center.top),
                    Slot::South => {
                        Rect::new(center.left, center.bottom, center.right, rect.bottom)
                    }
                    Slot::West => Rect::new(rect.left, center.top, center.left, center.bottom),
                    Slot::East => Rect::new(center.right, center.top, rect.right, center.bottom),
                    Slot::Center => center,
                };
                Some((widget, space))
            })
            .collect()
    }
}

impl Widget for LayoutNswe {
    fn base(&self) -> &WidgetBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut WidgetBase {
        &mut self.base
    }

    fn children(&self) -> Vec<WidgetId> {
        self.slots.iter().flatten().copied().collect()
    }

    fn arrange(&mut self, cx: &mut LayoutContext) -> Result<Vec<(WidgetId, Rect)>> {
        Ok(self.slot_rects(self.base.rect(), |id| {
            let widget = cx.widget(id)?;
            let base = widget.base();
            widget
                .takes_space()
                .then(|| (base.outer_width(), base.outer_height()))
        }))
    }
}

/// An empty widget taking space: a gap in a layout. It ignores the mouse, so clicks go
/// through it (C++ featherui's `distance`).
#[derive(Default)]
pub struct Spacer {
    base: WidgetBase,
}

impl Spacer {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Widget for Spacer {
    fn base(&self) -> &WidgetBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut WidgetBase {
        &mut self.base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rects(layout: &LayoutNswe, sizes: &[(WidgetId, Option<(f32, f32)>)]) -> Vec<Rect> {
        let size = |id: WidgetId| sizes.iter().find(|(w, _)| *w == id).and_then(|(_, s)| *s);
        layout
            .slot_rects(Rect::new(0.0, 0.0, 100.0, 80.0), size)
            .into_iter()
            .map(|(_, rect)| rect)
            .collect()
    }

    #[test]
    fn five_slots_share_the_space() {
        let [n, s, e, w, c] = [0, 1, 2, 3, 4].map(WidgetId::<Spacer>::new).map(|i| i.untyped());
        let layout = LayoutNswe::new()
            .with(Slot::North, n)
            .with(Slot::South, s)
            .with(Slot::East, e)
            .with(Slot::West, w)
            .with(Slot::Center, c);
        let sizes = [
            (n, Some((0.0, 10.0))),
            (s, Some((0.0, 20.0))),
            (e, Some((15.0, 0.0))),
            (w, Some((5.0, 0.0))),
            (c, Some((1.0, 1.0))),
        ];
        assert_eq!(
            rects(&layout, &sizes),
            [
                Rect::new(5.0, 0.0, 85.0, 10.0),   // north
                Rect::new(5.0, 60.0, 85.0, 80.0),  // south
                Rect::new(85.0, 10.0, 100.0, 60.0), // east
                Rect::new(0.0, 10.0, 5.0, 60.0),   // west
                Rect::new(5.0, 10.0, 85.0, 60.0),  // center
            ]
        );
    }

    #[test]
    fn empty_and_gone_slots_leave_their_space_to_the_center() {
        let (e, c) = (WidgetId::<Spacer>::new(0).untyped(), WidgetId::<Spacer>::new(1).untyped());
        let layout = LayoutNswe::new().with(Slot::East, e).with(Slot::Center, c);
        // East takes space, nothing else is there: north and south are full width.
        assert_eq!(
            rects(&layout, &[(e, Some((30.0, 0.0))), (c, Some((0.0, 0.0)))]),
            [Rect::new(70.0, 0.0, 100.0, 80.0), Rect::new(0.0, 0.0, 70.0, 80.0)]
        );
        // East is gone: it still gets a (zero-width) slot, and the center everything.
        assert_eq!(
            rects(&layout, &[(e, None), (c, Some((0.0, 0.0)))]),
            [Rect::new(100.0, 0.0, 100.0, 80.0), Rect::new(0.0, 0.0, 100.0, 80.0)]
        );
    }

    #[test]
    fn too_wide_sides_overlap_instead_of_inverting_the_center() {
        let (e, w) = (WidgetId::<Spacer>::new(0).untyped(), WidgetId::<Spacer>::new(1).untyped());
        let layout = LayoutNswe::new().with(Slot::East, e).with(Slot::West, w);
        let rects = rects(&layout, &[(e, Some((70.0, 0.0))), (w, Some((70.0, 0.0)))]);
        assert_eq!(rects[0], Rect::new(70.0, 0.0, 100.0, 80.0));
        assert_eq!(rects[1], Rect::new(0.0, 0.0, 70.0, 80.0));
    }
}
