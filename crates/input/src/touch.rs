//! Taps, swipes and the scroll wheel on the reader (TURN-1, TURN-2): which part of the screen a
//! tap hit, whether a pointer movement was a swipe, and when wheel movement turns a page.
//!
//! Tap zones are fixed (owner, 2026-11-01, as enScore): the reader is split into two equal halves,
//! left turns back and right turns forward. A thin band along the top shows or hides the
//! controls, so the whole page height below it turns.

use crate::Action;

/// Shortest horizontal travel, in frontend pixels, that counts as a swipe.
pub const MIN_SWIPE: f64 = 60.0;

/// Longest travel, in frontend pixels, that still counts as a tap.
pub const MAX_TAP_TRAVEL: f64 = 12.0;

/// The share of the reader's height, from the top, that shows or hides the controls.
pub const CONTROLS_BAND: f64 = 0.12;

/// Wheel travel, in frontend pixels, that turns one page.
pub const WHEEL_STEP: f64 = 60.0;

/// After a wheel turn, further wheel movement is ignored for this long, so one flick of a
/// trackpad turns one page.
pub const WHEEL_COOLDOWN_MS: u64 = 350;

/// The parts of the reader area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    /// Left half: turn back.
    Back,
    /// The band along the top: show or hide the controls.
    Controls,
    /// Right half: turn forward.
    Forward,
}

impl Zone {
    /// The reader action, if the zone has one.
    pub fn action(self) -> Option<Action> {
        match self {
            Zone::Back => Some(Action::Previous),
            Zone::Forward => Some(Action::Next),
            Zone::Controls => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Zone::Back => "back",
            Zone::Controls => "controls",
            Zone::Forward => "forward",
        }
    }
}

/// The zone a tap at `x`, `y` hits in an area `width` by `height`. A tap in the top
/// [`CONTROLS_BAND`] shows or hides the controls; otherwise the left half turns back and the
/// right half forward. `y` may be `None` (or the height unknown): halves only. Positions outside
/// the area clamp to its edges; a non-number `x` or an empty area hits the controls zone (no
/// turn by accident).
pub fn tap_zone(x: f64, y: Option<f64>, width: f64, height: f64) -> Zone {
    if !(x.is_finite() && width.is_finite() && width > 0.0) {
        return Zone::Controls;
    }
    if let Some(y) = y
        && y.is_finite()
        && height.is_finite()
        && height > 0.0
        && y / height < CONTROLS_BAND
    {
        return Zone::Controls;
    }
    if (x / width).clamp(0.0, 1.0) < 0.5 {
        Zone::Back
    } else {
        Zone::Forward
    }
}

/// Turns pages from scroll-wheel movement: wheel travel adds up until it reaches
/// [`WHEEL_STEP`], then one page turns (down: forward, up: back) and the wheel rests for
/// [`WHEEL_COOLDOWN_MS`].
#[derive(Debug, Clone, Default)]
pub struct WheelGate {
    travel: f64,
    resting_until: u64,
}

impl WheelGate {
    pub fn new() -> WheelGate {
        WheelGate::default()
    }

    /// `dy` pixels of wheel movement at `now_ms` (positive: content moves up, i.e. the wheel
    /// turned down). Returns the turn, if any.
    pub fn wheel(&mut self, dy: f64, now_ms: u64) -> Option<Action> {
        if !dy.is_finite() || dy == 0.0 {
            return None;
        }
        if now_ms < self.resting_until {
            self.travel = 0.0;
            return None;
        }
        if self.travel.signum() != dy.signum() {
            self.travel = 0.0;
        }
        self.travel += dy;
        if self.travel.abs() < WHEEL_STEP {
            return None;
        }
        let action = if self.travel > 0.0 {
            Action::Next
        } else {
            Action::Previous
        };
        self.travel = 0.0;
        self.resting_until = now_ms.saturating_add(WHEEL_COOLDOWN_MS);
        Some(action)
    }
}

/// What a finished pointer movement was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    Tap,
    /// Finger moved left: turn forward.
    SwipeLeft,
    /// Finger moved right: turn back.
    SwipeRight,
    /// A drag, a vertical swipe or anything else: no turn.
    Other,
}

impl Gesture {
    pub fn action(self) -> Option<Action> {
        match self {
            Gesture::SwipeLeft => Some(Action::Next),
            Gesture::SwipeRight => Some(Action::Previous),
            _ => None,
        }
    }
}

/// Classifies a movement of `dx`, `dy` pixels from press to release. While the page is zoomed in,
/// frontends pan instead and never call this.
pub fn classify(dx: f64, dy: f64) -> Gesture {
    if !(dx.is_finite() && dy.is_finite()) {
        return Gesture::Other;
    }
    if dx.hypot(dy) <= MAX_TAP_TRAVEL {
        return Gesture::Tap;
    }
    if dx.abs() >= MIN_SWIPE && dx.abs() > dy.abs() {
        return if dx < 0.0 {
            Gesture::SwipeLeft
        } else {
            Gesture::SwipeRight
        };
    }
    Gesture::Other
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_halves_and_a_controls_band() {
        let z = |x, y| tap_zone(x, y, 900.0, 600.0);
        assert_eq!(z(10.0, Some(300.0)), Zone::Back);
        assert_eq!(z(449.0, Some(300.0)), Zone::Back);
        assert_eq!(z(450.0, Some(300.0)), Zone::Forward);
        assert_eq!(z(899.0, Some(590.0)), Zone::Forward);
        assert_eq!(z(450.0, Some(30.0)), Zone::Controls, "the top band");
        assert_eq!(z(10.0, Some(30.0)), Zone::Controls);
        assert_eq!(z(450.0, None), Zone::Forward, "no y: halves only");
        assert_eq!(z(-50.0, Some(300.0)), Zone::Back);
        assert_eq!(z(5000.0, Some(300.0)), Zone::Forward);
        assert_eq!(z(f64::NAN, Some(300.0)), Zone::Controls);
        assert_eq!(z(10.0, Some(f64::NAN)), Zone::Back);
        assert_eq!(tap_zone(10.0, None, 0.0, 600.0), Zone::Controls);
        assert_eq!(Zone::Forward.action(), Some(Action::Next));
    }

    #[test]
    fn the_wheel_turns_one_page_per_step_and_rests() {
        let mut w = WheelGate::new();
        assert_eq!(w.wheel(30.0, 0), None);
        assert_eq!(w.wheel(30.0, 10), Some(Action::Next));
        // Resting: a trackpad's long flick doesn't turn again.
        assert_eq!(w.wheel(200.0, 100), None);
        assert_eq!(w.wheel(-100.0, 400), Some(Action::Previous));
        // A change of direction starts the count again.
        assert_eq!(w.wheel(50.0, 1000), None);
        assert_eq!(w.wheel(-50.0, 1010), None);
        assert_eq!(w.wheel(f64::NAN, 1020), None);
    }

    #[test]
    fn swipes_and_taps() {
        assert_eq!(classify(3.0, -4.0), Gesture::Tap);
        assert_eq!(classify(-120.0, 30.0), Gesture::SwipeLeft);
        assert_eq!(classify(80.0, -10.0), Gesture::SwipeRight);
        assert_eq!(classify(40.0, 0.0), Gesture::Other);
        assert_eq!(classify(70.0, 200.0), Gesture::Other);
        assert_eq!(classify(f64::INFINITY, 0.0), Gesture::Other);
        assert_eq!(Gesture::SwipeLeft.action(), Some(Action::Next));
    }
}
