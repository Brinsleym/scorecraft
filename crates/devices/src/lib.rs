//! scorecraft-devices: Platform services: MIDI ports, audio I/O, camera and keep-awake. The one crate where FFI may appear, behind Cargo features.
//!
//! Layer L4 in the workspace map (`docs/architecture.md` §2–3). M3 adds keep-awake ([`power`]);
//! MIDI, audio and camera come with their milestones.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

pub mod power;
pub mod sound;

pub use power::{KeepAwake, PowerError};
pub use sound::{ClickOut, SoundError};

/// The crate's package name, used by `cargo xtask layers` smoke tests and diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::CRATE_NAME, "scorecraft-devices");
    }
}
