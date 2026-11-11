/// A rectangle in UI units: `left..right` from left to right and `top..bottom` from top to
/// bottom (Y goes down, as for mouse positions).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Rect {
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    /// The rectangle with its top left corner at `(left, top)` and the given size.
    pub fn from_size(left: f32, top: f32, width: f32, height: f32) -> Self {
        Self::new(left, top, left + width, top + height)
    }

    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    /// Whether `(x, y)` is in the rectangle, edges included.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.left && x <= self.right && y >= self.top && y <= self.bottom
    }

    /// The rectangle made smaller by `margin` on every side. It doesn't go below zero size: an
    /// edge that would cross the opposite one stops at the middle.
    pub fn shrink(&self, margin: f32) -> Self {
        let (left, right) = shrink_span(self.left, self.right, margin);
        let (top, bottom) = shrink_span(self.top, self.bottom, margin);
        Self::new(left, top, right, bottom)
    }
}

fn shrink_span(start: f32, end: f32, margin: f32) -> (f32, f32) {
    if end - start >= 2.0 * margin {
        (start + margin, end - margin)
    } else {
        let middle = (start + end) / 2.0;
        (middle, middle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_and_hits() {
        let rect = Rect::from_size(10.0, 20.0, 30.0, 40.0);
        assert_eq!(rect, Rect::new(10.0, 20.0, 40.0, 60.0));
        assert_eq!((rect.width(), rect.height()), (30.0, 40.0));
        assert!(rect.contains(10.0, 20.0));
        assert!(rect.contains(40.0, 60.0));
        assert!(!rect.contains(9.9, 30.0));
        assert!(!rect.contains(20.0, 60.1));
    }

    #[test]
    fn shrinking_stops_at_the_middle() {
        let rect = Rect::new(0.0, 0.0, 100.0, 10.0);
        assert_eq!(rect.shrink(2.0), Rect::new(2.0, 2.0, 98.0, 8.0));
        assert_eq!(rect.shrink(6.0), Rect::new(6.0, 5.0, 94.0, 5.0));
    }
}
