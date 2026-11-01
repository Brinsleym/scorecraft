//! Palm rejection while the pen draws (spec `docs/spec/ink.md`, ANNO-14).
//!
//! The shell routes every pointer event through a [`PalmFilter`] before it reaches the reader or
//! the ink tools. While a pen touches the screen, touches are ignored, so a hand leaning on the
//! glass never turns, pans, zooms or draws. When no pen is down, touch passes untouched: taps,
//! swipes and two-finger pinches work as usual. The mouse always passes.

use std::collections::{BTreeMap, BTreeSet};

/// What made a pointer event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointerKind {
    /// A stylus tip.
    Pen,
    /// A stylus's eraser end.
    Eraser,
    Touch,
    Mouse,
}

impl PointerKind {
    pub fn is_pen(self) -> bool {
        matches!(self, PointerKind::Pen | PointerKind::Eraser)
    }

    pub fn id(self) -> &'static str {
        match self {
            PointerKind::Pen => "pen",
            PointerKind::Eraser => "eraser",
            PointerKind::Touch => "touch",
            PointerKind::Mouse => "mouse",
        }
    }

    pub fn from_id(id: &str) -> Option<PointerKind> {
        [
            PointerKind::Pen,
            PointerKind::Eraser,
            PointerKind::Touch,
            PointerKind::Mouse,
        ]
        .into_iter()
        .find(|k| k.id() == id)
    }
}

/// Where a pointer is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Down,
    Move,
    Up,
    /// The platform withdrew the pointer (on Android 13 and later, a palm).
    Cancel,
}

/// The filter's answer for one event.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filtered {
    /// Hand the event on to the reader or tool.
    pub pass: bool,
    /// Touch pointers whose gesture in progress must be cancelled now: a pending tap must not
    /// turn, a pan or pinch stops, and ink a touch started is removed.
    pub cancel: Vec<u64>,
}

/// The most pointers tracked at once; more are ignored (a hostile or broken driver).
pub const MAX_POINTERS: usize = 64;

/// A touch that starts this soon after the last pen lifts is a hand landing, not a tap (spec
/// ANNO-14): short enough that no deliberate tap is lost.
pub const PEN_LIFT_GRACE_MS: u64 = 50;

/// Tracks pens and touches to decide which touch events to drop.
#[derive(Debug, Clone, Default)]
pub struct PalmFilter {
    pens: BTreeSet<u64>,
    /// Touches down now, and whether each is being ignored.
    touches: BTreeMap<u64, bool>,
    /// When the last pen lifted (event time, milliseconds).
    pen_lifted_ms: Option<u64>,
}

impl PalmFilter {
    pub fn new() -> PalmFilter {
        PalmFilter::default()
    }

    /// Whether a pen is touching the screen.
    pub fn pen_down(&self) -> bool {
        !self.pens.is_empty()
    }

    /// Whether a touch starting at `now_ms` is ignored: a pen is down, or one lifted less than
    /// [`PEN_LIFT_GRACE_MS`] ago. A clock that went backwards counts as just after the lift.
    fn rejects_new_touch(&self, now_ms: u64) -> bool {
        self.pen_down()
            || self
                .pen_lifted_ms
                .is_some_and(|t| now_ms.saturating_sub(t) < PEN_LIFT_GRACE_MS)
    }

    /// Decides one event from pointer `id` of `kind`, at event time `now_ms` (milliseconds, any
    /// monotonic origin).
    pub fn event(&mut self, id: u64, kind: PointerKind, phase: Phase, now_ms: u64) -> Filtered {
        let pass = Filtered {
            pass: true,
            cancel: Vec::new(),
        };
        let drop = Filtered::default();
        match kind {
            PointerKind::Mouse => pass,
            PointerKind::Pen | PointerKind::Eraser => match phase {
                Phase::Down => {
                    if self.pens.len() < MAX_POINTERS {
                        self.pens.insert(id);
                    }
                    // Touches already down are a hand that landed first: cancel their gestures.
                    let cancel: Vec<u64> = self
                        .touches
                        .iter_mut()
                        .filter(|(_, ignored)| !**ignored)
                        .map(|(t, ignored)| {
                            *ignored = true;
                            *t
                        })
                        .collect();
                    Filtered { pass: true, cancel }
                }
                Phase::Move => pass,
                Phase::Up | Phase::Cancel => {
                    self.pens.remove(&id);
                    if self.pens.is_empty() {
                        self.pen_lifted_ms = Some(now_ms);
                    }
                    pass
                }
            },
            PointerKind::Touch => match phase {
                Phase::Down => {
                    let ignored = self.rejects_new_touch(now_ms);
                    if self.touches.len() < MAX_POINTERS || self.touches.contains_key(&id) {
                        self.touches.insert(id, ignored);
                    } else {
                        return drop;
                    }
                    if ignored { drop } else { pass }
                }
                Phase::Move => match self.touches.get(&id) {
                    Some(false) => pass,
                    Some(true) => drop,
                    // A move without a down (missed event): judge it by the pen now.
                    None => {
                        if self.pen_down() {
                            drop
                        } else {
                            pass
                        }
                    }
                },
                Phase::Up => match self.touches.remove(&id) {
                    Some(true) => drop,
                    Some(false) => pass,
                    None => {
                        if self.pen_down() {
                            drop
                        } else {
                            pass
                        }
                    }
                },
                Phase::Cancel => match self.touches.remove(&id) {
                    // The platform says this was a palm: undo what it started.
                    Some(false) => Filtered {
                        pass: false,
                        cancel: vec![id],
                    },
                    _ => drop,
                },
            },
        }
    }

    /// Forgets every pointer (the window lost focus, the score closed).
    pub fn reset(&mut self) {
        self.pens.clear();
        self.touches.clear();
        self.pen_lifted_ms = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use PointerKind::*;
    use std::cell::Cell;

    thread_local! {
        static NOW: Cell<u64> = const { Cell::new(1_000) };
    }

    /// A clock that moves a second per event, so the post-lift grace never applies by accident.
    fn tick() -> u64 {
        NOW.with(|n| {
            n.set(n.get() + 1_000);
            n.get()
        })
    }

    #[test]
    fn ink_20_touches_while_the_pen_draws_are_ignored() {
        let mut f = PalmFilter::new();
        assert!(f.event(1, Pen, Phase::Down, tick()).pass);
        assert!(!f.event(7, Touch, Phase::Down, tick()).pass);
        assert!(!f.event(7, Touch, Phase::Move, tick()).pass);
        assert!(f.event(1, Pen, Phase::Move, tick()).pass);
        // The pen lifts before the hand: the hand stays ignored until it lifts.
        assert!(f.event(1, Pen, Phase::Up, tick()).pass);
        assert!(!f.event(7, Touch, Phase::Move, tick()).pass);
        assert!(!f.event(7, Touch, Phase::Up, tick()).pass);
        // A new touch after that is a real one.
        assert!(f.event(8, Touch, Phase::Down, tick()).pass);
    }

    #[test]
    fn ink_21_pen_down_cancels_a_touch_in_progress() {
        let mut f = PalmFilter::new();
        assert!(f.event(3, Touch, Phase::Down, tick()).pass);
        let r = f.event(1, Pen, Phase::Down, tick());
        assert!(r.pass);
        assert_eq!(r.cancel, vec![3]);
        // Its lift is not a tap.
        assert!(!f.event(3, Touch, Phase::Up, tick()).pass);
    }

    #[test]
    fn ink_22_two_finger_pinch_without_a_pen() {
        let mut f = PalmFilter::new();
        for id in [1, 2] {
            assert!(f.event(id, Touch, Phase::Down, tick()).pass);
        }
        for _ in 0..5 {
            assert!(f.event(1, Touch, Phase::Move, tick()).pass);
            assert!(f.event(2, Touch, Phase::Move, tick()).pass);
        }
        assert!(f.event(1, Touch, Phase::Up, tick()).pass);
        assert!(f.event(2, Touch, Phase::Up, tick()).pass);
        // Pen, then zoom: no switch needed.
        f.event(9, Pen, Phase::Down, tick());
        f.event(9, Pen, Phase::Up, tick());
        assert!(f.event(4, Touch, Phase::Down, tick()).pass);
        assert!(f.event(5, Touch, Phase::Down, tick()).pass);
    }

    #[test]
    fn ink_23_platform_palm_cancel_undoes_the_touch() {
        let mut f = PalmFilter::new();
        f.event(6, Touch, Phase::Down, tick());
        let r = f.event(6, Touch, Phase::Cancel, tick());
        assert!(!r.pass);
        assert_eq!(r.cancel, vec![6]);
        assert_eq!(
            f.event(6, Touch, Phase::Cancel, tick()),
            Filtered::default()
        );
    }

    #[test]
    fn mouse_always_passes_and_eraser_counts_as_pen() {
        let mut f = PalmFilter::new();
        f.event(2, Eraser, Phase::Down, tick());
        assert!(f.pen_down());
        assert!(f.event(0, Mouse, Phase::Down, tick()).pass);
        assert!(!f.event(5, Touch, Phase::Down, tick()).pass);
        f.event(2, Eraser, Phase::Cancel, tick());
        assert!(!f.pen_down());
        f.reset();
        assert!(f.event(5, Touch, Phase::Move, tick()).pass);
    }

    #[test]
    fn ink_31_touch_just_after_the_pen_lifts_is_ignored() {
        let mut f = PalmFilter::new();
        f.event(1, Pen, Phase::Down, 10_000);
        f.event(1, Pen, Phase::Up, 10_100);
        assert!(
            !f.event(5, Touch, Phase::Down, 10_120).pass,
            "20 ms after: a landing hand"
        );
        assert!(!f.event(5, Touch, Phase::Up, 10_200).pass);
        assert!(
            f.event(6, Touch, Phase::Down, 10_300).pass,
            "200 ms after: a real tap"
        );
        // A clock that went backwards still counts as just after the lift.
        f.event(1, Pen, Phase::Down, 20_000);
        f.event(1, Pen, Phase::Up, 20_010);
        assert!(!f.event(7, Touch, Phase::Down, 19_000).pass);
        f.reset();
        assert!(f.event(8, Touch, Phase::Down, 19_000).pass);
    }

    #[test]
    fn floods_of_pointers_are_bounded() {
        let mut f = PalmFilter::new();
        for id in 0..1000 {
            f.event(id, Touch, Phase::Down, tick());
            f.event(id + 10_000, Pen, Phase::Down, tick());
        }
        assert!(f.touches.len() <= MAX_POINTERS);
        assert!(f.pens.len() <= MAX_POINTERS);
        assert_eq!(PointerKind::from_id("pen"), Some(Pen));
        assert_eq!(PointerKind::from_id("finger"), None);
    }
}
