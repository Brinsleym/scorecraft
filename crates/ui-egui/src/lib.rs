//! scorecraft-ui-egui: The egui shell: library, reader, tools and dialogs. Thin: every action goes through the engine.
//!
//! Layer L6 in the workspace map (`docs/architecture.md` §2–3, §16): built on the Craft suite's
//! `craft-ui` with the suite's egui. M3 brings the reader: [`App`] draws a toolbar and the
//! [`reader::ReaderCanvas`], and turns taps, swipes, keys and pedals into engine commands.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

pub mod app;
pub mod control;
pub mod ink;
pub mod library_view;
pub mod markup_bar;
pub mod reader;
pub mod score_panel;
pub mod settings;
pub mod state;
pub mod theme;

pub use app::{App, NoPlatform, Platform, SHELL_COMMANDS};
pub use settings::Settings;
pub use state::{Theme, UiState};
pub use theme::Tokens;

/// The crate's package name, used by `cargo xtask layers` smoke tests and diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::CRATE_NAME, "scorecraft-ui-egui");
    }
}
