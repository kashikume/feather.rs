//! Touches passed to the app as the mouse, as GuitarPrompter does on Android.
//!
//! The first finger down is followed until it lifts, as the mouse with its left button pressed;
//! other fingers are ignored meanwhile. Before the press, the pointer moves to where the finger
//! is, so that hover-based UI (featherui's buttons) finds what is under it; after the release,
//! the pointer leaves, so nothing stays highlighted under a lifted finger.

use winit::event::TouchPhase;

/// What a touch means for the app, in `FeatherApp` calls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MouseCall {
    /// `on_mouse_moved(x, y)`.
    Moved(f32, f32),
    /// `on_mouse_button(Left, pressed, x, y)`.
    Button { pressed: bool, x: f32, y: f32 },
    /// `on_mouse_left()`.
    Left,
}

/// Turns touches into mouse calls, following one finger at a time.
#[derive(Debug, Default)]
pub struct TouchAsMouse {
    /// The finger being followed (winit's touch `id`), while it is down.
    finger: Option<u64>,
}

impl TouchAsMouse {
    /// The mouse calls for finger `id` in `phase` at `(x, y)` (window pixels, from the top
    /// left). Empty for fingers other than the followed one.
    pub fn touch(&mut self, id: u64, phase: TouchPhase, x: f32, y: f32) -> Vec<MouseCall> {
        match phase {
            TouchPhase::Started if self.finger.is_none() => {
                self.finger = Some(id);
                vec![
                    MouseCall::Moved(x, y),
                    MouseCall::Button {
                        pressed: true,
                        x,
                        y,
                    },
                ]
            }
            TouchPhase::Moved if self.finger == Some(id) => vec![MouseCall::Moved(x, y)],
            // Cancelled: the system took the gesture over; released all the same, as
            // GuitarPrompter treats `CANCEL` as up.
            TouchPhase::Ended | TouchPhase::Cancelled if self.finger == Some(id) => {
                self.finger = None;
                vec![
                    MouseCall::Button {
                        pressed: false,
                        x,
                        y,
                    },
                    MouseCall::Left,
                ]
            }
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use MouseCall::{Button, Left};
    use TouchPhase::{Cancelled, Ended, Started};

    fn press(x: f32, y: f32) -> MouseCall {
        Button {
            pressed: true,
            x,
            y,
        }
    }

    fn release(x: f32, y: f32) -> MouseCall {
        Button {
            pressed: false,
            x,
            y,
        }
    }

    #[test]
    fn a_tap_moves_presses_releases_and_leaves() {
        let mut touch = TouchAsMouse::default();
        assert_eq!(
            touch.touch(1, Started, 10.0, 20.0),
            vec![MouseCall::Moved(10.0, 20.0), press(10.0, 20.0)]
        );
        assert_eq!(
            touch.touch(1, Ended, 10.0, 20.0),
            vec![release(10.0, 20.0), Left]
        );
    }

    #[test]
    fn a_drag_moves_the_pointer() {
        let mut touch = TouchAsMouse::default();
        touch.touch(1, Started, 10.0, 20.0);
        assert_eq!(
            touch.touch(1, TouchPhase::Moved, 15.0, 30.0),
            vec![MouseCall::Moved(15.0, 30.0)]
        );
        assert_eq!(
            touch.touch(1, Ended, 16.0, 31.0),
            vec![release(16.0, 31.0), Left]
        );
    }

    #[test]
    fn other_fingers_are_ignored_while_one_is_down() {
        let mut touch = TouchAsMouse::default();
        touch.touch(1, Started, 10.0, 20.0);
        assert!(touch.touch(2, Started, 50.0, 60.0).is_empty());
        assert!(touch.touch(2, TouchPhase::Moved, 55.0, 65.0).is_empty());
        assert!(touch.touch(2, Ended, 55.0, 65.0).is_empty());
        assert_eq!(
            touch.touch(1, TouchPhase::Moved, 11.0, 21.0),
            vec![MouseCall::Moved(11.0, 21.0)]
        );
    }

    #[test]
    fn the_next_finger_is_followed_after_the_first_lifts() {
        let mut touch = TouchAsMouse::default();
        touch.touch(1, Started, 10.0, 20.0);
        touch.touch(2, Started, 50.0, 60.0);
        touch.touch(1, Ended, 10.0, 20.0);
        // Finger 2 went down while 1 was followed: still ignored until it lifts.
        assert!(touch.touch(2, TouchPhase::Moved, 51.0, 61.0).is_empty());
        assert_eq!(
            touch.touch(3, Started, 70.0, 80.0),
            vec![MouseCall::Moved(70.0, 80.0), press(70.0, 80.0)]
        );
    }

    #[test]
    fn cancelled_releases() {
        let mut touch = TouchAsMouse::default();
        touch.touch(1, Started, 10.0, 20.0);
        assert_eq!(
            touch.touch(1, Cancelled, 12.0, 22.0),
            vec![release(12.0, 22.0), Left]
        );
        assert!(touch.touch(1, TouchPhase::Moved, 13.0, 23.0).is_empty());
    }

    #[test]
    fn moves_and_ends_without_a_start_are_ignored() {
        let mut touch = TouchAsMouse::default();
        assert!(touch.touch(1, TouchPhase::Moved, 10.0, 20.0).is_empty());
        assert!(touch.touch(1, Ended, 10.0, 20.0).is_empty());
    }
}
