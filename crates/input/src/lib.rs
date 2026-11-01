//! scorecraft-input: Pedal key maps, MIDI learn, pen shortcuts and palm rejection, mapped to commands.
//!
//! Layer L4 in the workspace map (`docs/architecture.md` §2–3). Frontends turn platform events
//! into the plain values here; this crate decides which reader action they mean
//! (`docs/spec/page-turning.md`, TURN-1..4) and the engine dispatches the action's command.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

mod keys;
mod palm;
mod touch;

pub use keys::{
    Action, Chord, DEFAULT_DEBOUNCE_MS, KeyGate, MAX_ENTRIES, PROFILES, PedalMap, Profile,
};
pub use palm::{Filtered, MAX_POINTERS, PEN_LIFT_GRACE_MS, PalmFilter, Phase, PointerKind};
pub use touch::{Gesture, MAX_TAP_TRAVEL, MIN_SWIPE, Zone, classify, tap_zone};

/// The crate's package name, used by `cargo xtask layers` smoke tests and diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::CRATE_NAME, "scorecraft-input");
    }
}
