//! scorecraft-engine: the session façade every frontend talks to: command registry and dispatch,
//! jobs, events, view-models and preferences.
//!
//! Layer L5 (`docs/architecture.md` §4). Every user-visible action is a command with a stable id
//! from `docs/feature-map.md`; the UI, CLI, control channel, MCP, pedals and MIDI all call
//! [`Session::dispatch`].
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

mod annotate_cmds;
pub mod commands;
mod cover_cmds;
mod edit_cmds;
mod engine_cmds;
mod error;
mod export_cmds;
pub mod feature_catalog;
mod find_cmds;
mod fulltext_cmds;
mod input_cmds;
mod layer_cmds;
mod library;
mod library_cmds;
mod list_cmds;
mod markup;
mod metronome_cmds;
mod nav_cmds;
mod page_cmds;
pub mod parity;
mod play_cmds;
mod reader;
mod reader_cmds;
mod score_cmds;
mod select_cmds;
mod session;
mod snapshot;
mod stamp_cmds;
mod text_cmds;
mod view_cmds;

pub use commands::{CommandInfo, CommandSpec, registry};
pub use cover_cmds::CoverJob;
pub use error::EngineError;
pub use library::{AnyStorage, Clock, library_open};
pub use reader::{FrameSlot, ShownPage};
pub use session::Session;
pub use snapshot::{compose, unturn};

/// The crate's package name, used in diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::CRATE_NAME, "scorecraft-engine");
    }
}
