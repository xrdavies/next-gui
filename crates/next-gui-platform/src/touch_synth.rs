//! P6a: synthesize primary-finger touch into pointer events for HMI.
//!
//! Retained UI already handles [`Event::PointerMoved`] / [`Event::PointerButton`].
//! Touchscreens become usable without dual UI handlers by mapping the **primary**
//! contact into that path while still emitting [`Event::Touch`] for apps that want
//! raw multi-touch later.
//!
//! Coordinates stay in **physical pixels** (same as the touch stub). Logical /
//! DPI conversion is P6b.
//!
//! ## Double-fire with OS mouse
//!
//! Some stacks (especially X11) also emit `CursorMoved` / `MouseInput` for the
//! same finger. Backends should suppress those while [`TouchPointerSynth::has_primary`]
//! is true. Suppression is best-effort: a late mouse-up after primary release, or
//! a second pointing device during touch, may still produce an extra pointer event.

use crate::{Event, PointerButton, TouchEvent, TouchPhase, WindowId};

/// Tracks the primary touch id and emits pointer events for HMI.
#[derive(Debug, Default, Clone)]
pub struct TouchPointerSynth {
    /// Active primary contact id, if any.
    primary_id: Option<u64>,
}

impl TouchPointerSynth {
    /// Empty synthesizer (no primary contact).
    pub fn new() -> Self {
        Self::default()
    }

    /// True while a primary finger is down (Started… before Ended/Cancelled).
    ///
    /// Backends use this to skip OS-synthesized mouse events for the same contact.
    pub fn has_primary(&self) -> bool {
        self.primary_id.is_some()
    }

    /// Primary contact id, if currently tracking one.
    pub fn primary_id(&self) -> Option<u64> {
        self.primary_id
    }

    /// Map one touch sample into zero or more pointer events (delivery order).
    ///
    /// - Primary `Started` → `PointerMoved` then `PointerButton { Left, pressed: true }`
    /// - Primary `Moved` → `PointerMoved`
    /// - Primary `Ended` / `Cancelled` → `PointerMoved` then `PointerButton { Left, pressed: false }`
    ///   (Moved keeps the release position aligned; Cancelled must not leave a sticky press)
    /// - Secondary fingers → no pointer synthesis
    pub fn synthesize(&mut self, window: WindowId, touch: TouchEvent) -> Vec<Event> {
        match touch.phase {
            TouchPhase::Started => {
                if self.primary_id.is_some() {
                    return Vec::new();
                }
                self.primary_id = Some(touch.id);
                vec![
                    Event::PointerMoved {
                        window,
                        x: touch.x,
                        y: touch.y,
                    },
                    Event::PointerButton {
                        window,
                        button: PointerButton::Left,
                        pressed: true,
                        x: touch.x,
                        y: touch.y,
                    },
                ]
            }
            TouchPhase::Moved => {
                if self.primary_id != Some(touch.id) {
                    return Vec::new();
                }
                vec![Event::PointerMoved {
                    window,
                    x: touch.x,
                    y: touch.y,
                }]
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                if self.primary_id != Some(touch.id) {
                    return Vec::new();
                }
                self.primary_id = None;
                vec![
                    Event::PointerMoved {
                        window,
                        x: touch.x,
                        y: touch.y,
                    },
                    Event::PointerButton {
                        window,
                        button: PointerButton::Left,
                        pressed: false,
                        x: touch.x,
                        y: touch.y,
                    },
                ]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(id: u64, phase: TouchPhase, x: f64, y: f64) -> TouchEvent {
        TouchEvent { id, phase, x, y }
    }

    fn assert_moved(ev: &Event, x: f64, y: f64) {
        match ev {
            Event::PointerMoved {
                window,
                x: mx,
                y: my,
            } => {
                assert_eq!(*window, WindowId(1));
                assert_eq!(*mx, x);
                assert_eq!(*my, y);
            }
            other => panic!("expected PointerMoved, got {other:?}"),
        }
    }

    fn assert_button(ev: &Event, pressed: bool, x: f64, y: f64) {
        match ev {
            Event::PointerButton {
                window,
                button,
                pressed: p,
                x: bx,
                y: by,
            } => {
                assert_eq!(*window, WindowId(1));
                assert_eq!(*button, PointerButton::Left);
                assert_eq!(*p, pressed);
                assert_eq!(*bx, x);
                assert_eq!(*by, y);
            }
            other => panic!("expected PointerButton, got {other:?}"),
        }
    }

    #[test]
    fn primary_started_moved_ended_order() {
        let mut s = TouchPointerSynth::new();
        let w = WindowId(1);

        let start = s.synthesize(w, touch(10, TouchPhase::Started, 1.0, 2.0));
        assert_eq!(start.len(), 2);
        assert_moved(&start[0], 1.0, 2.0);
        assert_button(&start[1], true, 1.0, 2.0);
        assert!(s.has_primary());
        assert_eq!(s.primary_id(), Some(10));

        let moved = s.synthesize(w, touch(10, TouchPhase::Moved, 5.0, 6.0));
        assert_eq!(moved.len(), 1);
        assert_moved(&moved[0], 5.0, 6.0);

        let end = s.synthesize(w, touch(10, TouchPhase::Ended, 7.0, 8.0));
        assert_eq!(end.len(), 2);
        assert_moved(&end[0], 7.0, 8.0);
        assert_button(&end[1], false, 7.0, 8.0);
        assert!(!s.has_primary());
    }

    #[test]
    fn cancelled_releases_press() {
        let mut s = TouchPointerSynth::new();
        let w = WindowId(1);
        let _ = s.synthesize(w, touch(1, TouchPhase::Started, 0.0, 0.0));
        assert!(s.has_primary());
        let cancel = s.synthesize(w, touch(1, TouchPhase::Cancelled, 3.0, 4.0));
        assert_eq!(cancel.len(), 2);
        assert_button(&cancel[1], false, 3.0, 4.0);
        assert!(!s.has_primary());
        // A late Ended for the same id must not synthesize again.
        let late = s.synthesize(w, touch(1, TouchPhase::Ended, 9.0, 9.0));
        assert!(late.is_empty());
    }

    #[test]
    fn secondary_finger_does_not_synthesize() {
        let mut s = TouchPointerSynth::new();
        let w = WindowId(1);
        let _ = s.synthesize(w, touch(1, TouchPhase::Started, 0.0, 0.0));

        let second_start = s.synthesize(w, touch(2, TouchPhase::Started, 10.0, 10.0));
        assert!(second_start.is_empty());
        let second_move = s.synthesize(w, touch(2, TouchPhase::Moved, 11.0, 11.0));
        assert!(second_move.is_empty());
        let second_end = s.synthesize(w, touch(2, TouchPhase::Ended, 12.0, 12.0));
        assert!(second_end.is_empty());

        // Primary still active and can move.
        let primary_move = s.synthesize(w, touch(1, TouchPhase::Moved, 1.0, 1.0));
        assert_eq!(primary_move.len(), 1);
        assert_moved(&primary_move[0], 1.0, 1.0);

        let primary_end = s.synthesize(w, touch(1, TouchPhase::Ended, 1.0, 1.0));
        assert_eq!(primary_end.len(), 2);
        assert_button(&primary_end[1], false, 1.0, 1.0);
    }

    #[test]
    fn new_primary_after_release() {
        let mut s = TouchPointerSynth::new();
        let w = WindowId(1);
        let _ = s.synthesize(w, touch(1, TouchPhase::Started, 0.0, 0.0));
        let _ = s.synthesize(w, touch(1, TouchPhase::Ended, 0.0, 0.0));
        let again = s.synthesize(w, touch(99, TouchPhase::Started, 4.0, 5.0));
        assert_eq!(again.len(), 2);
        assert_button(&again[1], true, 4.0, 5.0);
        assert_eq!(s.primary_id(), Some(99));
    }

    #[test]
    fn moved_without_primary_is_ignored() {
        let mut s = TouchPointerSynth::new();
        let orphan = s.synthesize(WindowId(1), touch(7, TouchPhase::Moved, 1.0, 1.0));
        assert!(orphan.is_empty());
    }
}
